#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""`Arc::get_mut` ban in the layout crate (Spec 09 L9-016).

Spec 09 S9-1 made `ParaCache` hand out `Arc<ParagraphLayout>`, so the shaping
cache and the page editing index share one allocation. Mutating a shared layout
must therefore go through `Arc::make_mut`, which takes a private copy. That much
is enforced by the type: `Arc<T>` yields `&T`, so a forgotten copy is an E0596
borrow error rather than a silent cross-placement corruption (R9-14).

`Arc::get_mut` is the one shape that defeats it:

    if let Some(l) = Arc::get_mut(&mut layout) { l.items.push(item) }

That compiles, returns `None` whenever the value is shared — which, for a cached
layout, is always — and reports nothing. The mutation is silently skipped. It is
the same failure as a sentinel that cannot distinguish "nothing to do" from "I
could not do it" (L9-009), and this gate exists because the shape was predicted
before anyone wrote it, rather than found after an incident.

`get_mut` on `Vec`, `HashMap`, `RefCell` and friends is unaffected — only the
`Arc::` associated function is matched, which is the only form `Arc::get_mut`
can be written in (it is deliberately not a method, so `.get_mut()` never
resolves to it).

Scope: `loki-layout/src/**.rs`, where layout sharing is load-bearing. Widen
`SCOPES` if another crate starts sharing `Arc`s that are mutated in place.

Comments are stripped before matching, on purpose. The suppression ratchet
text-matches `let _ =` anywhere in a file including prose, so documenting the
pattern it polices trips it (Spec 08 §8). A gate that cannot be explained in the
code it guards is a gate people work around; this one lets you name the hazard
in a doc comment and still fails on a real call.

Usage:
    scripts/check-arc-get-mut.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Crates where `Arc` sharing is load-bearing and in-place mutation is a hazard.
SCOPES = ("loki-layout/src/",)

# `Arc::get_mut`, however the path is spelled: `Arc::`, `sync::Arc::`, or a
# fully-qualified `std::sync::Arc::`.
GET_MUT = re.compile(r"\bArc::get_mut\b")

# Everything from `//` to end of line. Crude — it also truncates a `//` inside a
# string literal — but this gate only ever needs the code *before* a comment, so
# over-truncating can produce a false negative on a pathological line, never a
# false positive on prose. Erring that way is deliberate: a gate that fires on
# documentation gets disabled.
LINE_COMMENT = re.compile(r"//.*$")


def in_scope(rel: str) -> bool:
    return rel.endswith(".rs") and any(rel.startswith(s) for s in SCOPES)


def main() -> int:
    out = subprocess.check_output(["git", "ls-files", "*.rs"], cwd=REPO, text=True)
    failures: list[str] = []
    scanned = 0
    for rel in out.splitlines():
        if not rel or not in_scope(rel):
            continue
        scanned += 1
        text = (REPO / rel).read_text(encoding="utf-8", errors="replace")
        for i, line in enumerate(text.splitlines(), 1):
            if GET_MUT.search(LINE_COMMENT.sub("", line)):
                failures.append(f"{rel}:{i}")

    if failures:
        print(f"Arc::get_mut gate: {len(failures)} violation(s):\n")
        for v in failures:
            print(f"  ✗ {v}: `Arc::get_mut` on a shared layout")
        print(
            "\n`Arc::get_mut` returns None whenever the value is shared, so this\n"
            "silently skips the mutation instead of taking a private copy. Use\n"
            "`Arc::make_mut`, which clones on write and cannot fail (Spec 09\n"
            "L9-016, R9-14). If you genuinely need the fallible form, the value\n"
            "is not a shared layout and does not belong in this crate's scope."
        )
        return 1

    print(
        f"Arc::get_mut gate: OK — {scanned} file(s) in "
        f"{', '.join(SCOPES)}; copy-on-write goes through Arc::make_mut."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
