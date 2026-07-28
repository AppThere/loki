#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Document rendering must not depend on the UI's text scale (Spec 08 I-24/r51).

Why this exists
---------------
I-24 will make chrome respond to the platform's accessibility text-size setting,
by expressing UI tokens in `rem` and setting `font-size` on the document root.
Spec 08 draws the boundary: **the setting applies to chrome, not to document
content.** A 12pt paragraph is 12pt — that is document fidelity, and the user's
lever there is zoom.

The reason that boundary is worth a gate rather than a convention is Phase 2.
Texture residency scales with *rendered page pixels*: demand is
`page_px x zoom x display_scale`, bounded against a survival ceiling that §3.6b
measured as reachable on **every** memory size from 2 to 64 GiB. Let a text-scale
term reach document rendering and a 2.0x setting multiplies page area by ~4 on
the exact axis an entire phase was spent bounding — the ceiling would begin
firing at ordinary zoom, and the symptom (soft text at rest) would look like a
memory regression rather than like an accessibility change.

Two paths could carry the term across, so both are checked:

  1. **A crate dependency.** If a document-path crate could see
     `appthere-ui`, a token conversion could reach layout or tile planning
     directly. Today none of them does — the references that exist are doc
     comments explaining that values are *injected* instead — and this keeps it
     that way.
  2. **CSS inheritance into the canvas subtree.** `rem`/`em`/`ch`/`lh` are
     font-relative: a canvas-adjacent element sized in them would scale the
     document surface with the root font size, with no crate dependency
     involved. The editor's canvas files must stay in device units.

This gate is cheap and structural. It cannot prove the whole boundary — a value
copied by hand across the line would pass — but it closes the two mechanisms
that make the crossing *easy*, which is what a convention leaves open.

Usage:
    scripts/check-document-scale-isolation.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Crates on the document rendering path: model, layout, paint, tile planning.
# A dependency from any of these on the UI token crate is the first crossing.
DOCUMENT_PATH_CRATES = [
    "loki-doc-model",
    "loki-layout",
    "loki-primitives",
    "loki-vello",
    "loki-renderer",
    "appthere-canvas",
    "loki-pdf",
]

UI_CRATE = re.compile(r"^\s*appthere[-_]ui\s*=", re.MULTILINE)

# Font-relative CSS units, as they appear inside a Rust style string.
FONT_RELATIVE = re.compile(r"\d(?:\.\d+)?\s*(rem|em|ch|lh|rlh|ex|cap|ic)\b")

# The editor's document surface and everything that sizes it.
CANVAS_GLOB = "loki-text/src/routes/editor/editor_canvas*.rs"


def crate_dependency_failures() -> list[str]:
    out: list[str] = []
    for crate in DOCUMENT_PATH_CRATES:
        manifest = REPO / crate / "Cargo.toml"
        if not manifest.exists():
            out.append(f"{crate}/Cargo.toml is missing — update this gate's list")
            continue
        if UI_CRATE.search(manifest.read_text(encoding="utf-8")):
            out.append(
                f"{crate} now depends on appthere-ui. Document rendering must not "
                f"see UI tokens: once those are in `rem` (I-24), a 2.0x text "
                f"setting would reach page pixels and multiply texture demand by "
                f"~4 against the Phase 2 survival ceiling. Inject the value "
                f"instead — see loki-renderer/src/view_types.rs for the pattern"
            )
    return out


def canvas_unit_failures() -> list[str]:
    out: list[str] = []
    files = subprocess.check_output(
        ["git", "ls-files", CANVAS_GLOB], cwd=REPO, text=True
    ).split()
    if not files:
        out.append(f"no files matched {CANVAS_GLOB} — the gate is pointing at nothing")
    for rel in files:
        text = (REPO / rel).read_text(encoding="utf-8", errors="replace")
        for num, line in enumerate(text.splitlines(), 1):
            stripped = line.lstrip()
            if stripped.startswith("//"):
                continue  # Prose may name the units it is forbidding.
            m = FONT_RELATIVE.search(line)
            if m:
                out.append(
                    f"{rel}:{num}: font-relative unit `{m.group(1)}` in the "
                    f"document canvas subtree. The surface must stay in device "
                    f"units — a rem-sized canvas scales page pixels with the "
                    f"root font size, which is the same Phase 2 hazard by a "
                    f"different path: {stripped[:60]}"
                )
    return out


def main() -> int:
    failures = crate_dependency_failures() + canvas_unit_failures()
    if failures:
        print(f"Document scale isolation: {len(failures)} violation(s):\n")
        for f in failures:
            print(f"  ✗ {f}")
        return 1
    print(
        f"Document scale isolation: OK — {len(DOCUMENT_PATH_CRATES)} document-path "
        f"crates independent of appthere-ui, canvas subtree in device units."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
