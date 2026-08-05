#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""ADR-0017 §5.6: per-run **advances**, DOM against canvas.

Reads the shot `scripts/sitting/run.sh advances` captures, measures each white
row — the probe sizes each row `fit-content` on one line, so its width *is* the
run's advance in CSS px — and sets it against the canvas path's advance for the
same run, which `styled_linebreak_lines` prints in points.

Points are converted at 96/72, the ratio Blitz resolves `pt` at, so both columns
are CSS px and the difference is directly readable.

What the two scales are for
---------------------------

A **rounding** difference is a fixed number of pixels: it stays put as the type
grows. A **metrics** difference is a fraction: it grows with it. Run this at
scale 1 and at scale 8 and compare the `delta` column against the `delta/scale`
one — whichever holds still is the answer.

Usage
-----

    scripts/sitting/advance_bands.py \\
        --shot target/sitting/adv.png --log target/sitting/adv.log \\
        --canvas target/sitting/adv-canvas.txt
"""

import argparse
import re
import subprocess
import sys
import tempfile

PT_TO_PX = 96.0 / 72.0
# The probe paints white rows on a blue page. The UA's 8 px body margin puts the
# page's left edge here, and a qualifying row starts exactly there — which is
# also what tells a row apart from the white margin strips, since those start at
# x = 0 and run the full width.
PAGE_LEFT = 8
# Narrower than any run in the fixture, wider than any stray artefact.
MIN_ROW_PX = 30


def read_ppm(path):
    """Decodes a binary PPM into `(width, height, bytes)`."""
    data = open(path, "rb").read()
    pos, fields = 0, []
    while len(fields) < 4:
        while data[pos : pos + 1].isspace():
            pos += 1
        if data[pos : pos + 1] == b"#":
            while data[pos : pos + 1] != b"\n":
                pos += 1
            continue
        start = pos
        while not data[pos : pos + 1].isspace():
            pos += 1
        fields.append(data[start:pos])
    return int(fields[1]), int(fields[2]), data[pos + 1 :]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--shot", required=True)
    ap.add_argument("--log", required=True, help="the probe's stdout (case names)")
    ap.add_argument("--canvas", required=True, help="styled_linebreak_lines output")
    args = ap.parse_args()

    with tempfile.NamedTemporaryFile(suffix=".ppm") as tmp:
        subprocess.run(["convert", args.shot, "-depth", "8", "ppm:" + tmp.name], check=True)
        width, height, px = read_ppm(tmp.name)

    def is_blue(x, y):
        i = (y * width + x) * 3
        return px[i] < 60 and px[i + 1] < 60 and px[i + 2] > 200

    def is_black(x, y):
        i = (y * width + x) * 3
        return px[i] < 40 and px[i + 1] < 40 and px[i + 2] < 40

    def row_width(y):
        """Distance from the page's left edge to the first blue pixel, or 0.

        Measured from the *blue* side rather than the white: the UA's body margin
        paints white in the 8 px around the page, so a white run starting at the
        row would be indistinguishable from one starting at the margin — and the
        row's own glyphs are not white either. The first blue pixel is the row
        box's right edge and nothing else.
        """
        for x in range(PAGE_LEFT, width):
            if is_blue(x, y):
                w = x - PAGE_LEFT
                return w if w >= MIN_ROW_PX else 0
        # No blue on this image row: either it is above/below the window, or it
        # is one of the body-margin strips. Not a row box either way. (Black is
        # *not* a terminator: the row's own glyphs are black, and stopping at the
        # first one split every band in two.)
        return 0

    names, scale = [], 1.0
    for line in open(args.log):
        if line.startswith("case "):
            names.append(line.split()[1])
        elif line.startswith("scale "):
            scale = float(line.split()[1])

    # The canvas path's advances, in document order — one line per paragraph,
    # because each advance case is a single run that does not wrap.
    canvas_pt = [
        float(m.group(1))
        for m in re.finditer(r"advance=\s*([0-9.]+)", open(args.canvas).read())
    ]

    # A band is a run of image rows that all carry a qualifying white row. The
    # probe's gap separates one case from the next. The band's width is the
    # widest such row: a row box is a rectangle, so every line of it is the same
    # width, and taking the max means a stray antialiased edge row cannot shorten
    # it.
    bands, y = [], 0
    while y < height:
        w = row_width(y)
        if w:
            top, widest = y, 0
            while y < height and row_width(y):
                widest = max(widest, row_width(y))
                y += 1
            bands.append((top, widest))
        else:
            y += 1

    print(f"image {width}x{height}, scale {scale}")
    print(f"{len(bands)} bands, {len(names)} cases, {len(canvas_pt)} canvas advances")
    if not (len(bands) == len(names) == len(canvas_pt)):
        print("COUNT MISMATCH — the three inputs are not the same run")
        return 1
    if any(PAGE_LEFT + w >= width - 1 for _, w in bands):
        print("a band reaches the last column — the window clipped a run; widen it")
        return 1

    print(
        f"\n{'case':>10} {'canvas px':>10} {'dom px':>8} {'delta':>8} "
        f"{'delta/scale':>12} {'delta %':>8}"
    )
    for name, (_, dom), pt in zip(names, bands, canvas_pt):
        canvas = pt * PT_TO_PX
        delta = dom - canvas
        print(
            f"{name:>10} {canvas:>10.2f} {dom:>8} {delta:>8.2f} "
            f"{delta / scale:>12.3f} {100 * delta / canvas:>7.3f}%"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
