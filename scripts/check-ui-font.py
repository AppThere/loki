#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""UI typeface guard (Spec 08 r94/r95).

The UI face is declared **once**, at the document root, by
`appthere_ui::ui_font_css()`. Everything below inherits it. This gate forbids a
component from declaring `font-family` again.

Why a gate rather than a convention
-----------------------------------
The family used to be opt-in per component — 91 copies of one fact across four
crates — and the failure that produced was not a wrong copy but a *missing* one:
anything mounted outside such a component (everything `AtPopoverHost` renders)
fell through to the CSS initial value and drew in **serif**. The fix was to
declare it at the root and delete the copies.

A deleted duplicate comes back. Two of the 91 had already drifted to
`system-ui, sans-serif`, which is not the bundled face at all and which no
review caught, because a `font-family` line looks like diligence. So the rule
that keeps this true has to be mechanical: **no `font-family` in a component**,
with the deliberate exceptions named in an allowlist rather than judged by
whoever is reading.

What is legitimately excepted
-----------------------------
A face that is deliberately *not* the UI face: monospace for code and formula
entry, and the spreadsheet's italic-serif row marker. Those go in
`scripts/ui-font-allowlist.txt` as `path | family` with a reason, so adding one
is a decision someone writes down.

Keyed on the *family*, not the line number. A line-number allowlist would churn
against every unrelated edit to a 1000-line editor file, and a gate that fails
for reasons unrelated to its subject is a gate people learn to re-baseline
without reading.

Usage:
    scripts/check-ui-font.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ALLOWLIST_FILE = REPO / "scripts" / "ui-font-allowlist.txt"

# The UI crates. Document *content* typography (loki-layout, the export crates)
# is a different subject and is not scoped here.
SCOPED_DIRS = (
    "appthere-ui/src/",
    "loki-text/src/",
    "loki-spreadsheet/src/",
    "loki-presentation/src/",
)

# The one file allowed to say it: the root sheet itself.
THE_DECLARATION = "appthere-ui/src/ui_font.rs"

DECL = re.compile(r"font-family\s*:\s*([^;\"]+)")


def load_allowlist() -> set[str]:
    if not ALLOWLIST_FILE.exists():
        return set()
    out = set()
    for raw in ALLOWLIST_FILE.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            out.add(line)
    return out


def main() -> int:
    # The report uses non-ASCII glyphs (✗, —); force UTF-8 output so a violation
    # message does not itself crash on a cp1252 console (Windows-local runs).
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    allow = load_allowlist()
    violations: list[str] = []
    seen: set[str] = set()

    for d in SCOPED_DIRS:
        for path in sorted((REPO / d).rglob("*.rs")):
            rel = path.relative_to(REPO).as_posix()
            if rel == THE_DECLARATION or rel.endswith("_tests.rs"):
                continue
            for n, line in enumerate(
                path.read_text(encoding="utf-8", errors="replace").splitlines(), 1
            ):
                # Prose about the property is not a declaration of it.
                stripped = line.lstrip()
                if stripped.startswith("//"):
                    continue
                m = DECL.search(line)
                if not m:
                    continue
                key = f"{rel} | {m.group(1).strip()}"
                seen.add(key)
                if key not in allow:
                    violations.append(f"  ✗ {rel}:{n} — {m.group(1).strip()}")

    # A stale allowlist entry is the other half: an exception that no longer
    # exists reads as a sanctioned deviation and hides that the code changed.
    stale = sorted(allow - seen)

    if violations:
        print(f"UI-font gate: {len(violations)} component declaration(s) of font-family:")
        print(*violations, sep="\n")
        print()
        print("  The UI face is declared once, by appthere_ui::ui_font_css(), and")
        print("  inherited. Delete the declaration. If this element genuinely needs a")
        print("  different face (monospace, say), add it to scripts/ui-font-allowlist.txt")
        print("  with the reason.")
    if stale:
        print(f"UI-font gate: {len(stale)} stale allowlist entry(ies) — the line moved or went away:")
        for s in stale:
            print(f"  ✗ {s}")
    if violations or stale:
        return 1

    print(f"UI-font gate: OK — one declaration, {len(allow)} allowed exception(s).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
