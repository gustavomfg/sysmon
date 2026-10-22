#!/usr/bin/env python3
"""Bounded Linux PTY smoke: real telemetry, resize, focus, expansion and exit.

Uses only Python's standard library. Does not save captured system information.
Usage: python3 tools/smoke_graphics_terminal.py [target/release/sysmon]
"""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import termios
import time


def smoke(binary, ascii_mode):
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)

    def resize(width, height):
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))

    def enter_terminal():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    resize(160, 45)
    env = dict(os.environ, TERM="xterm-256color",
               SYSMON_COLOR="mono" if ascii_mode else "truecolor",
               SYSMON_GRAPHICS="ascii" if ascii_mode else "unicode")
    process = subprocess.Popen([str(binary)], stdin=slave, stdout=slave, stderr=slave,
                               env=env, preexec_fn=enter_terminal)
    events = [(1, b"\t\t\te"), (2, b"e"), (3, (80, 24)), (4, (120, 35)),
              (5, b"\x10theme\r"), (6, b"/sysmon"), (6.5, b"\x1b"), (7, b"q")]
    start = time.monotonic()
    output = bytearray()
    completed = 0
    try:
        while process.poll() is None and time.monotonic() - start < 12:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                output.extend(chunk)
                # Crossterm requests a cursor position from the terminal.
                if b"\x1b[6n" in chunk:
                    os.write(master, b"\x1b[1;1R")
            elapsed = time.monotonic() - start
            while completed < len(events) and elapsed >= events[completed][0]:
                action = events[completed][1]
                if isinstance(action, tuple):
                    resize(*action)
                else:
                    os.write(master, action)
                completed += 1
        if process.poll() is None:
            process.wait(timeout=2)
        while select.select([master], [], [], 0)[0]:
            try:
                output.extend(os.read(master, 65536))
            except OSError:
                break
        result = {
            "mode": "ascii/mono" if ascii_mode else "unicode/truecolor",
            "events_sent": completed,
            "exit_code": process.returncode,
            "alternate_screen_restored": b"\x1b[?1049l" in output,
            "raw_mode_restored": termios.tcgetattr(slave) == original,
            "rendered_bytes": len(output),
        }
        assert completed == len(events) and process.returncode == 0, result
        assert result["alternate_screen_restored"] and result["raw_mode_restored"], result
        return result
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=2)
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/release/sysmon").resolve()
    for ascii_mode in [False, True]:
        print(json.dumps(smoke(binary, ascii_mode)), flush=True)
