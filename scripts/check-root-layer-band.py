#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Reserved z-index band for root layers (Spec 08 r65).

Why this exists
---------------
The spelling menu's migration left its old backdrop behind at `z-index: 1000`,
inside an editor root that is `position: relative` with no `z-index` — so no
stacking context. It competed directly with the root-hosted popover at 41, won,
and produced **a menu that looked correct and dismissed on every click meant to
use it**.

Nothing caught it. Not the type system, not any gate, not a test — the failure is
a paint-order relationship between two files that never mention each other, and
it is only visible on a screen. That is the worst available failure shape, so the
remedy is to make the collision *unavailable* rather than documented (L08-043).

The rule
--------
`appthere_ui::components::overlay::BACKDROP_Z_INDEX` (40) and up is the **root
layer band**: the backdrop and the popover host, ordered between themselves by
DOM order because `z-index` cannot arbitrate between two children of one
positioned root.

The design system owns that band — the ribbon's overflow menu sits at 41
deliberately, so its controls stay clickable above the backdrop, and the modal
dialogs sit at 2000+ so they cover everything. **Application crates may not enter
it at all.** An app-crate value at or above the floor is by construction
competing with a root layer it cannot see, which is exactly what happened.

An app surface that needs to stack locally has the whole range below 40, which is
40 more levels than any one of them has ever used.

Usage:
    scripts/check-root-layer-band.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

from gate_source import is_prose_line

REPO = Path(__file__).resolve().parent.parent

# `appthere_ui::components::overlay::BACKDROP_Z_INDEX`. Duplicated as a literal
# because this gate reads text rather than linking the crate; the constant's own
# docs point here, so the two cannot drift silently.
ROOT_LAYER_FLOOR = 40

# The crates that consume the design system. `appthere-ui` owns the band and is
# deliberately absent.
APP_CRATES = ("loki-text/", "loki-spreadsheet/", "loki-presentation/")

Z_INDEX = re.compile(r"z-index:\s*(\d+)")


def main() -> int:
    out = subprocess.check_output(["git", "ls-files", "*.rs"], cwd=REPO, text=True)
    failures: list[str] = []
    scanned = 0

    for rel in out.split():
        if not rel.startswith(APP_CRATES):
            continue
        scanned += 1
        text = (REPO / rel).read_text(encoding="utf-8", errors="replace")
        for num, line in enumerate(text.splitlines(), 1):
            # Comments only: the z-index values live *inside* style strings, so
            # blanking string literals here would blind the gate to its subject.
            if is_prose_line(line):
                continue
            for m in Z_INDEX.finditer(line):
                value = int(m.group(1))
                if value >= ROOT_LAYER_FLOOR:
                    failures.append(
                        f"{rel}:{num}: z-index {value} is in the root layer band "
                        f"(>= {ROOT_LAYER_FLOOR}), which application crates may "
                        f"not enter. A value here competes with the backdrop and "
                        f"the popover host in a stacking context it cannot see — "
                        f"the r64 regression, where a leftover backdrop at 1000 "
                        f"painted over a menu at 41 and swallowed every click. "
                        f"Stack locally below {ROOT_LAYER_FLOOR}, or use the "
                        f"popover host if the thing belongs at the root."
                    )

    if failures:
        print(f"Root layer band: {len(failures)} violation(s):\n")
        for f in failures:
            print(f"  ✗ {f}")
        return 1

    print(
        f"Root layer band: OK — no application-crate z-index at or above "
        f"{ROOT_LAYER_FLOOR} across {scanned} files."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
