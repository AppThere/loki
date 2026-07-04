#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""TODO-format gate (Spec 01 audit A-11 / CLAUDE.md annotations).

Every deferred-work marker in first-party production code must be a tagged
`// TODO(<topic>):` so it stays greppable by topic (see the inventory in
docs/adr/spec-01-todo-compat-inventory.md). This gate fails on:

  * a bare `// TODO` / `// TODO:` with no `(topic)`;
  * any `// FIXME` / `// HACK` / `// XXX` (CLAUDE.md uses `TODO(<topic>)` and
    `COMPAT(<target>)`; these other markers are not part of the convention).

Scope: first-party `<crate>/src/**.rs`. Excluded: `tests/`, `*_tests.rs`,
`*/tests.rs`, `benches/`, `examples/`, `patches/*`. `COMPAT(<target>)` markers
are a separate, sanctioned convention and are not checked here.

Usage:
    scripts/check-todo-format.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# A bare TODO comment: `// TODO` not immediately followed by `(`.
BARE_TODO = re.compile(r"//+\s*TODO(?!\s*\()")
OTHER_MARKERS = re.compile(r"//+\s*(FIXME|HACK|XXX)\b")


def is_excluded(rel: str) -> bool:
    return (
        "/src/" not in rel
        or "/tests/" in rel
        or rel.endswith("_tests.rs")
        or rel.endswith("/tests.rs")
        or "/benches/" in rel
        or "/examples/" in rel
        or rel.startswith("patches/")
    )


def main() -> int:
    out = subprocess.check_output(["git", "ls-files", "*.rs"], cwd=REPO, text=True)
    failures: list[str] = []
    for rel in out.splitlines():
        if not rel or is_excluded(rel):
            continue
        for i, line in enumerate(
            (REPO / rel).read_text(encoding="utf-8", errors="replace").splitlines(), 1
        ):
            if BARE_TODO.search(line):
                failures.append(f"{rel}:{i}: bare TODO — use `// TODO(<topic>): …`")
            if OTHER_MARKERS.search(line):
                m = OTHER_MARKERS.search(line).group(1)
                failures.append(
                    f"{rel}:{i}: `{m}` — convert to `// TODO(<topic>): …`"
                )

    if failures:
        print(f"TODO-format gate: {len(failures)} violation(s):\n")
        for v in failures:
            print(f"  ✗ {v}")
        print("\nTag every deferred note as `// TODO(<topic>): …` so it stays "
              "tracked (docs/adr/spec-01-todo-compat-inventory.md).")
        return 1

    print("TODO-format gate: OK — all TODOs are `TODO(<topic>)`-tagged; "
          "no FIXME/HACK/XXX.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
