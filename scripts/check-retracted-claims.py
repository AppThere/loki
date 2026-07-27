#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Retracted-claim sweep (Spec 08 L08-032, generalised).

A correction in a ledger loses to a wrong story at the point of use.

Spec 08 retracted the "tight leading" account of I-06 and filed it. One turn
later the same account was restated, in a review of a different subject, by the
author of the retraction — because the retraction lived in a sweep test nobody
opens first, while the wrong version lived in the module docs of the file named
for the bug and in the status registry. Those are the two places a reader lands.
The correction was never competing on equal terms.

Remembering to search on retraction is the same class of control this program has
already watched fail: it gets skipped exactly when attention is on the new thing.
So the search runs mechanically. `scripts/retracted-claims.txt` lists the
phrasings; this fails if any appears somewhere that does not mark it as
retracted.

Marking a site as knowing better
--------------------------------
A phrase is allowed when any of `MARKERS` appears in the same paragraph —
"retracted", "refuted", "falsified", "L08-032", "was wrong", "superseded", "do
not infer". So the retraction itself passes, and so does any doc that quotes
the wrong version in order to correct it. That is deliberate: the remedy for a
wrong claim is a visible correction next to it, not deletion, because a phrase
that merely disappears teaches nobody.

Usage:
    scripts/check-retracted-claims.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

CLAIMS_FILE = Path("scripts/retracted-claims.txt")

# How far from a hit a marker may sit and still cover it.
#
# The unit that matters is the *paragraph* — a reader takes the correction from
# the prose around the phrase, not from a fixed number of lines — so the window
# is bounded by blank lines where there are any, and falls back to this many
# lines where there are not (a table row, a dense list). Eight was calibrated
# against the real sites: three lines missed five correctly-marked paragraphs and
# would have trained people to disable the gate.
CONTEXT_LINES = 8

MARKERS = (
    "retract",
    "refut",
    "falsif",
    "l08-032",
    "was wrong",
    "is wrong",
    "wrong version",
    "supersede",
    "do not infer",
    "no longer",
    "renamed",
    "false finding",
    "measurement artifact",
    "ordering artifact",
)

SUFFIXES = {".rs", ".md", ".toml", ".py", ".sh", ".ftl"}

# Paths where a listed phrase is somebody else's vocabulary rather than a copy of
# our retracted claim: vendored upstream code, and the ZIP-limit modules that use
# `over_budget` for an unrelated and correct purpose.
EXCLUDED_PREFIXES = (
    "patches/blitz-",
    "patches/dioxus-",
    "patches/blitz_",
    "loki-odf/src/limits.rs",
    "loki-opc/src/zip/limits.rs",
    "scripts/retracted-claims.txt",
    "scripts/check-retracted-claims.py",
)


def tracked_files() -> list[Path]:
    out = subprocess.run(
        ["git", "ls-files"], capture_output=True, text=True, check=True
    ).stdout
    files = []
    for line in out.splitlines():
        if not line or Path(line).suffix not in SUFFIXES:
            continue
        if any(line.startswith(p) for p in EXCLUDED_PREFIXES):
            continue
        files.append(Path(line))
    return files


def load_claims() -> list[tuple[re.Pattern[str], str]]:
    claims = []
    for raw in CLAIMS_FILE.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        pattern, _, why = line.partition("\t")
        if not why:
            print(f"{CLAIMS_FILE}: no tab-separated reason on: {line}", file=sys.stderr)
            sys.exit(2)
        claims.append((re.compile(pattern.strip(), re.IGNORECASE), why.strip()))
    return claims


def is_blank(line: str) -> bool:
    """A paragraph break, in prose or in a `//!` / `///` doc comment."""
    return line.strip().lstrip("/!").strip() == ""


def covered(lines: list[str], index: int) -> bool:
    """Whether a correction sits in the same paragraph as the hit at `index`."""
    lo = index
    while lo > 0 and index - lo < CONTEXT_LINES and not is_blank(lines[lo - 1]):
        lo -= 1
    hi = index
    while hi + 1 < len(lines) and hi - index < CONTEXT_LINES and not is_blank(lines[hi + 1]):
        hi += 1
    window = " ".join(lines[lo : hi + 1]).lower()
    return any(m in window for m in MARKERS)


def main() -> int:
    if not CLAIMS_FILE.exists():
        print(f"missing {CLAIMS_FILE}", file=sys.stderr)
        return 2
    claims = load_claims()
    findings: list[str] = []
    for path in tracked_files():
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (UnicodeDecodeError, OSError):
            continue
        for i, line in enumerate(lines):
            for pattern, why in claims:
                if pattern.search(line) and not covered(lines, i):
                    findings.append(f"{path}:{i + 1}: {pattern.pattern}\n    {why}")

    if findings:
        print("Retracted claims restated without a correction beside them:\n")
        for f in findings:
            print(f"  {f}\n")
        print(
            "Each site either states a claim that was withdrawn, or uses a name\n"
            "that was renamed. Fix the text, or — if the site is quoting the wrong\n"
            "version in order to correct it — say so within "
            f"{CONTEXT_LINES} lines, using one\n"
            "of: " + ", ".join(MARKERS) + ".\n"
        )
        return 1

    print(f"check-retracted-claims: {len(claims)} claims, no unmarked restatements")
    return 0


if __name__ == "__main__":
    sys.exit(main())
