#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Reserved z-index band for root layers (Spec 08 r65).

Why this exists — and the correction that changed the reason
------------------------------------------------------------
r64 introduced this gate on an account that is **retracted (r66)**: that the
spelling menu's leftover backdrop at `z-index: 1000` "competed directly" with
the root-hosted popover at 41 and won, because the editor root creates no
stacking context.

Blitz has no stacking contexts anywhere:
`paint_children` is each parent's own layout children (`blitz-dom`
`document.rs`), sorted by `z_index()` among *siblings only* (`layout/damage.rs`),
painted by walking that list (`blitz-paint` `render.rs`) and hit-tested by
walking it in reverse (`node.rs` `hit`). A descendant's 1000 never meets a root
sibling's 41. The predicted symptom could not have happened.

What did happen, on screen, is stronger and points the same way: a root-hosted
backdrop left up by mistake swallowed every click in the application — editor,
scrollbar, tab bar — because nothing below the root can outrank a root sibling
at any z-index. The cause was a lifetime defect, fixed in
`popover/anchor_scope.rs`; this gate is about the band.

The rule
--------
`appthere_ui::components::overlay::BACKDROP_Z_INDEX` (40) and up is the **root
layer band**: the backdrop and the popover host, ordered between themselves by
DOM order, since they are siblings with adjacent values.

The design system owns that band; the modal dialogs sit at 2000+.
**Application crates may not enter it at all** — not because an app value would
win, but because it *cannot*. A root layer outranks every application surface
unconditionally, so a value up here is always a consumer that has misunderstood
where its overlay lives, and the correct fix is to host it (`AtPopoverHost`)
rather than to raise it.

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
                        f"not enter. Blitz sorts z-index among siblings only, so "
                        f"a value here cannot outrank the root-hosted backdrop or "
                        f"popover host — it just marks a surface that has "
                        f"misunderstood where its overlay lives. Stack locally "
                        f"below {ROOT_LAYER_FLOOR}, or use the popover host if "
                        f"the thing belongs at the root."
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
