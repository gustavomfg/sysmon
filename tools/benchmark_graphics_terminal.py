#!/usr/bin/env python3
"""Bounded Linux PTY benchmark using real telemetry; prints JSON, not screen data.

CPU is relative to one core. Warmup is two seconds. Run variants sequentially
and alternate their order to reduce bias. No system settings are changed.
"""
import argparse
import collections
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import statistics
import struct
import subprocess
import termios
import time


def measure(binary, width, height, seconds):
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))

    def enter_terminal():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    env = dict(os.environ, TERM="xterm-256color", SYSMON_COLOR="truecolor",
               SYSMON_GRAPHICS="unicode")
    process = subprocess.Popen([str(binary)], stdin=slave, stdout=slave,
                               stderr=subprocess.PIPE, env=env, preexec_fn=enter_terminal)
    started = time.monotonic()
    samples = []
    diagnostics = bytearray()
    output_bytes = 0
    restored = False
    quitting = False
    ticks = os.sysconf("SC_CLK_TCK")
    page_kib = os.sysconf("SC_PAGE_SIZE") / 1024
    tail = b""

    def drain(fd):
        nonlocal output_bytes, restored, tail
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            return
        if fd == master:
            output_bytes += len(chunk)
            combined = tail + chunk
            restored |= b"\x1b[?1049l" in combined
            if b"\x1b[6n" in combined:
                os.write(master, b"\x1b[1;1R")
                combined = combined.rsplit(b"\x1b[6n", 1)[1]
            tail = combined[-16:]
        else:
            diagnostics.extend(chunk[:max(0, 1_048_576 - len(diagnostics))])

    try:
        while process.poll() is None and time.monotonic() - started < seconds + 3:
            ready, _, _ = select.select([master, process.stderr.fileno()], [], [], 0.05)
            for fd in ready:
                drain(fd)
            elapsed = time.monotonic() - started
            if not quitting and elapsed >= 2 and (not samples or elapsed - samples[-1][0] >= 0.25):
                try:
                    fields = Path(f"/proc/{process.pid}/stat").read_text().rsplit(")", 1)[1].split()
                    samples.append((elapsed, (int(fields[11]) + int(fields[12])) / ticks,
                                    int(fields[21]) * page_kib))
                except FileNotFoundError:
                    pass
            if elapsed >= seconds and not quitting:
                os.write(master, b"q")
                quitting = True
        if process.poll() is None:
            raise RuntimeError("SysMon did not exit within the bounded interval")
        # Slave is still open for checking raw-mode restoration; no EOF loop.
        while select.select([master], [], [], 0)[0]:
            drain(master)
        diagnostics.extend(process.stderr.read(max(0, 1_048_576 - len(diagnostics))))
        if process.returncode != 0 or len(samples) < 2:
            raise RuntimeError(f"Incomplete run: exit={process.returncode}, samples={len(samples)}")
        first, last = samples[0], samples[-1]
        result = dict(terminal=f"{width}x{height}", duration_seconds=seconds,
                      measurement_seconds=round(last[0] - first[0], 3),
                      cpu_one_core_percent=round((last[1] - first[1]) / (last[0] - first[0]) * 100, 2),
                      rss_min_kib=min(row[2] for row in samples),
                      rss_max_kib=max(row[2] for row in samples),
                      exit_code=process.returncode, alternate_screen_restored=restored,
                      raw_mode_restored=termios.tcgetattr(slave) == original,
                      output_bytes=output_bytes)
        assert restored and result["raw_mode_restored"], result
        # Optional evidence from an instrumented research build. The normal
        # application does not emit these lines or incur profiling overhead.
        stages = []
        for line in diagnostics.decode(errors="replace").splitlines():
            if line.startswith("PULSE_PROFILE,"):
                stages.append([int(value) for value in line.split(",")[1:]])
        if stages:
            buckets = collections.Counter(row[0] // 100 for row in stages)
            names = ["motion", "noise", "flow", "transport", "structure", "refinement", "lighting"]
            result["pulse_profile"] = dict(
                generations=len(stages), duplicate_tick_generations=sum(n - 1 for n in buckets.values()),
                stage_median_us={name: statistics.median(row[i + 1] for row in stages)
                                 for i, name in enumerate(names)},
                stage_total_us={name: sum(row[i + 1] for row in stages)
                                for i, name in enumerate(names)})
        return result
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=2)
        process.stderr.close()
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", nargs="?", type=Path, default=Path("target/release/sysmon"))
    parser.add_argument("--width", type=int, choices=range(20, 301), default=160, metavar="20..300")
    parser.add_argument("--height", type=int, choices=range(10, 101), default=45, metavar="10..100")
    parser.add_argument("--seconds", type=int, choices=range(4, 31), default=8, metavar="4..30")
    args = parser.parse_args()
    print(json.dumps(measure(args.binary.resolve(), args.width, args.height, args.seconds)), flush=True)
