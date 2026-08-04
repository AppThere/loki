#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""The measuring half of ADR-0017's styled-document line-break comparison.

Reads the screenshot `scripts/sitting/run.sh styledlinebreak` captures and the
stdout of `loki-text`'s `styled_linebreak_probe`, and prints each column's line
count so it can be set against `styled_linebreak_sweep`'s layout-side counts.

How a line count is read
------------------------

Each column is one render of the document at one width, painting its own white
background on a blue page. `line-height` is pinned by the probe, so a column's
white band is `constant + lines x LINE_PX` — the constant being top/bottom
padding plus the paragraph margins, neither of which depends on width.

The constant is calibrated once, against the layout-side count at the width
given by `--calibrate` (width and count). Every other band's count then has to
come out of the *same* constant. That is the comparison: not that some constant
exists, but that one constant reproduces all of them.

Bands are found in the image, not from arithmetic over the probe's widths: the
page carries the UA's 8 px body margin, which shifts every band right, and a
version that trusted the arithmetic sampled the gaps between columns.

Usage
-----

    scripts/sitting/linebreak_bands.py \
        --shot target/sitting/slb.png --log target/sitting/slb.log \
        --calibrate 586:8
"""

import argparse
import subprocess
import sys
import tempfile

# The probe's pinned `line-height`, and its column padding (24pt/18pt).
LINE_PX = 24
PAD_PX = 48
# A row inside every column's top padding: white in each band, never a glyph.
# 8 px of body margin, then a few px into the 32 px padding.
PROBE_ROW = 13
# Bands are hundreds of px wide; anything narrower is the body margin's sliver.
MIN_BAND_PX = 100


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
    ap.add_argument("--log", required=True)
    ap.add_argument(
        "--calibrate",
        required=True,
        metavar="WIDTH:LINES",
        help="a width and the layout side's line count there",
    )
    args = ap.parse_args()

    cal_w, cal_n = (int(v) for v in args.calibrate.split(":"))

    with tempfile.NamedTemporaryFile(suffix=".ppm") as tmp:
        subprocess.run(["convert", args.shot, "-depth", "8", "ppm:" + tmp.name], check=True)
        width, height, px = read_ppm(tmp.name)

    def white(x, y):
        i = (y * width + x) * 3
        return px[i] > 240 and px[i + 1] > 240 and px[i + 2] > 240

    widths = [
        int(line.split()[1]) for line in open(args.log) if line.startswith("band ")
    ]

    runs, x = [], 0
    while x < width:
        if white(x, PROBE_ROW):
            start = x
            while x < width and white(x, PROBE_ROW):
                x += 1
            if x - start > MIN_BAND_PX:
                runs.append((start, x - start))
        else:
            x += 1

    print(f"image {width}x{height}: {len(runs)} bands, {len(widths)} expected")
    if len(runs) != len(widths):
        print("BAND COUNT MISMATCH — the shot is not the row this expects")
        return 1

    bands = []
    for w, (bx, bw) in zip(widths, runs):
        # 12 px into the band: inside the column's 24 px left padding, so every
        # row of the band is white there and none of them is a glyph.
        bottom = max(y for y in range(height) if white(bx + 12, y))
        bands.append((w, bx, bw, bottom + 1))

    cal = [b for b in bands if b[0] == cal_w]
    if not cal:
        print(f"--calibrate width {cal_w} is not one of the bands")
        return 1
    const = cal[0][3] - cal_n * LINE_PX
    print(f"constant = {const} px (from w={cal_w} at {cal_n} lines)\n")

    ok = True
    print(f"{'width':>6} {'band px':>8} {'lines':>6}")
    for w, bx, bw, h in bands:
        exact = (h - const) % LINE_PX == 0
        lines = (h - const) / LINE_PX
        flag = "" if exact else "   NOT A WHOLE NUMBER OF LINES"
        ok &= exact
        print(f"{w:>6} {h:>8} {lines:>6}{flag}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
