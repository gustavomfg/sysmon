#!/usr/bin/env python3
"""Render the real ratatui cell fixtures to PNG. Development-only: needs Pillow.

Usage: python3 tools/render_graphics_capture.py CAPTURE_DIRECTORY
The captures contain explicitly synthetic telemetry from graphics_capture.rs.
"""
from pathlib import Path
import sys
from PIL import Image, ImageDraw, ImageFont

FONT_PAIRS = [
    ("/usr/share/fonts/TTF/MesloLGLNerdFontMono-Regular.ttf",
     "/usr/share/fonts/TTF/MesloLGLNerdFontMono-Bold.ttf"),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
     "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"),
]
pair = next((pair for pair in FONT_PAIRS if all(Path(p).exists() for p in pair)), None)
if pair is None:
    raise SystemExit("A supported monospace font is required; .cells/.txt captures remain usable.")
normal, bold = (ImageFont.truetype(path, 16) for path in pair)
for path in Path(sys.argv[1]).glob("*.cells"):
    rows = path.read_text().splitlines()
    width, height = map(int, rows[0].split())
    cell_width, cell_height, margin = 10, 21, 20
    canvas = Image.new("RGB", (width * cell_width + margin * 2,
                               height * cell_height + margin * 2), tuple(map(int, rows[-1].split("|")[2].split(","))))
    draw = ImageDraw.Draw(canvas)
    for index, row in enumerate(rows[1:]):
        codepoints, fg, bg, strong = row.split("|")
        text = "".join(chr(int(codepoint, 16)) for codepoint in codepoints.split(","))
        foreground, background = (tuple(map(int, value.split(","))) for value in (fg, bg))
        x = margin + (index % width) * cell_width
        y = margin + (index // width) * cell_height
        draw.rectangle((x, y, x + cell_width - 1, y + cell_height - 1), fill=background)
        draw.text((x, y + 2), text, font=bold if strong == "1" else normal, fill=foreground)
    canvas.save(path.with_suffix(".png"))
