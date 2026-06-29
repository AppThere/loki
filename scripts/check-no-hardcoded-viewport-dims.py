#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Hardcoded-viewport-dimension guard (Spec 01 audit A-1 / §6.2).

Forbids bare **screen-resolution-class** numeric literals (1280, 1920, 1024, …)
in the editor's input / hit-test / viewport code paths. These paths must take
their width/height from the measured `Viewport` / `scroll_metrics`, never from
an assumed window size — that assumption was the 1280px bug (A-1). This makes
that bug class un-reintroducible in the files where it lived.

This is the pragmatic, stable-toolchain stand-in for the `no_hardcoded_layout_dims`
**dylint** the spec envisions (§6.2). A true dylint does AST-level analysis on a
pinned nightly via the dylint driver — infrastructure not present in this CI — so
it remains a deferred specialist task; this gate covers the specific, high-value
class over the scoped hot-paths in the meantime.

A named `const`/`static` is the sanctioned alternative (e.g.
`DEFAULT_VIEWPORT_HEIGHT_PX`); literals on a `const`/`static` definition line,
in comments, or in test files are ignored. Genuine exceptions go in
`scripts/viewport-dims-allowlist.txt` (path:line, with a justification comment).

Usage:
    scripts/check-no-hardcoded-viewport-dims.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ALLOWLIST_FILE = REPO / "scripts" / "viewport-dims-allowlist.txt"

# Scoped to the editor input / hit-test / viewport paths where the 1280 bug
# lived and where dimensions must come from the measured Viewport.
SCOPED_DIRS = (
    "loki-text/src/routes/editor/",
    "loki-text/src/editing/",
)

# Common display-resolution widths/heights. A bare literal from this set in a
# layout path is almost always an assumed window size, not an intrinsic value.
DIMS = [
    640, 720, 768, 800, 960, 1024, 1080, 1136, 1280, 1366, 1440, 1536, 1600,
    1680, 1920, 2048, 2560, 3440, 3840, 4096,
]
LITERAL = re.compile(
    r"(?<![\w.])(?:" + "|".join(str(d) for d in DIMS) + r")(?:\.0+)?(?:_?f(?:32|64))?(?![\w.])"
)
CONST_DEF = re.compile(r"^\s*(pub(?:\([^)]*\))?\s+)?(const|static)\s")


def is_test(rel: str) -> bool:
    return rel.endswith("_tests.rs") or rel.endswith("/tests.rs") or "/tests/" in rel


def load_allowlist() -> set[str]:
    if not ALLOWLIST_FILE.exists():
        return set()
    out = set()
    for line in ALLOWLIST_FILE.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            out.add(line)
    return out


def main() -> int:
    allow = load_allowlist()
    out = subprocess.check_output(["git", "ls-files", "*.rs"], cwd=REPO, text=True)
    failures: list[str] = []
    scanned = 0
    for rel in out.splitlines():
        if not rel or is_test(rel) or not rel.startswith(SCOPED_DIRS):
            continue
        scanned += 1
        for i, line in enumerate(
            (REPO / rel).read_text(encoding="utf-8", errors="replace").splitlines(), 1
        ):
            code = line.split("//", 1)[0]
            if not LITERAL.search(code):
                continue
            if CONST_DEF.match(line):  # a named constant definition is allowed
                continue
            if f"{rel}:{i}" in allow:
                continue
            failures.append(f"{rel}:{i}: {line.strip()[:90]}")

    if failures:
        print(f"Viewport-dimension guard: {len(failures)} violation(s) "
              f"({scanned} scoped files):\n")
        for v in failures:
            print(f"  ✗ {v}")
        print("\nDimensions in the editor input/viewport paths must come from the "
              "measured `Viewport` / `scroll_metrics`, not an assumed screen size "
              "(audit A-1). Name a `const` for an intrinsic default, or add a "
              "justified entry to scripts/viewport-dims-allowlist.txt.")
        return 1

    print(f"Viewport-dimension guard: OK — {scanned} scoped files, "
          f"no bare screen-dimension literals.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
