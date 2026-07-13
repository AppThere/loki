#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Suppression ratchet (audit Q-3/Q-4 — error-swallows + `#[allow]`).

Two forms of swept-under-the-rug debt are counted per production `.rs` file and
frozen in `scripts/suppressions-baseline.txt` as a **ratchet** — the same model
as `check-file-ceiling.py`:

  * `let _ = …`  — a discarded binding. In the writers these are deliberate
    (in-memory `quick-xml` writes cannot fail), but each one is also a swallowed
    `Result`; we cap them so the count can only fall.
  * `#[allow(…)]` — a lint suppression. CLAUDE.md permits narrowly-scoped,
    justified allows, so we do not forbid them outright — we freeze the current
    population and require every *new* one to be a conscious `--update`.

Ratchet rules (per file, per metric):

  * a file NOT in the baseline must have **0** of each — new code starts clean;
  * a baselined file may keep its debt but must NOT grow either count;
  * when a file drops to **0 / 0** it must be removed from the baseline;
  * a baseline entry for a missing/renamed file fails (keeps the list honest).

So the two debts monotonically shrink toward empty, and adding a genuinely
needed suppression is a deliberate, reviewable act (`--update`, then explain it
in the diff) rather than something that silently accretes.

Scope: first-party production `.rs`. Excluded (as in the file-ceiling gate):
`tests/`, `*_tests.rs`, `*/tests.rs`, `benches/`, `examples/`, and `patches/*`.

Usage:
    scripts/check-suppressions.py            # enforce
    scripts/check-suppressions.py --update   # rewrite the baseline from current
                                             # counts (review the diff!)
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BASELINE_FILE = REPO / "scripts" / "suppressions-baseline.txt"

# `let _ = …` (optionally typed: `let _: T = …`). Anchored at a statement
# boundary so `slet _ =` / identifiers ending in "let" do not match.
LET_UNDERSCORE = re.compile(r"(?<![\w])let\s+_\s*(?::[^=]+)?=")
# `#[allow(` and `#![allow(` (inner attribute) — count both.
ALLOW = re.compile(r"#!?\[allow\(")


def is_test(rel: str) -> bool:
    return (
        "/tests/" in rel
        or rel.endswith("_tests.rs")
        or rel.endswith("/tests.rs")
        or "/benches/" in rel
        or "/examples/" in rel
        or rel.startswith("patches/")
    )


def counts_for(text: str) -> tuple[int, int]:
    return len(LET_UNDERSCORE.findall(text)), len(ALLOW.findall(text))


def production_files() -> dict[str, tuple[int, int]]:
    out = subprocess.check_output(["git", "ls-files", "*.rs"], cwd=REPO, text=True)
    counts: dict[str, tuple[int, int]] = {}
    for rel in out.splitlines():
        if not rel or is_test(rel):
            continue
        text = (REPO / rel).read_text(encoding="utf-8", errors="replace")
        counts[rel] = counts_for(text)
    return counts


def load_baseline() -> dict[str, tuple[int, int]]:
    base: dict[str, tuple[int, int]] = {}
    if not BASELINE_FILE.exists():
        return base
    for line in BASELINE_FILE.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        lets, allows, path = line.split(None, 2)
        base[path.strip()] = (int(lets), int(allows))
    return base


def write_baseline(counts: dict[str, tuple[int, int]]) -> None:
    debted = sorted(
        (
            (lets, allows, p)
            for p, (lets, allows) in counts.items()
            if lets or allows
        ),
        key=lambda t: (-(t[0] + t[1]), t[2]),
    )
    total_lets = sum(t[0] for t in debted)
    total_allows = sum(t[1] for t in debted)
    lines = [
        "# Suppression baseline — pre-existing `let _ =` swallows + `#[allow]` "
        "(Q-3/Q-4).",
        "# Format: `<let_underscore_count> <allow_count> <path>`. Ratcheted by",
        "# scripts/check-suppressions.py: neither count may GROW; a file must be",
        "# removed once both reach 0. New files must start at 0/0.",
        "# Regenerate with: scripts/check-suppressions.py --update",
        f"# Totals: {total_lets} `let _ =`, {total_allows} `#[allow]` across "
        f"{len(debted)} files.",
        "",
    ]
    lines += [f"{lets} {allows} {p}" for lets, allows, p in debted]
    BASELINE_FILE.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(
        f"Wrote {len(debted)} entries to "
        f"{BASELINE_FILE.relative_to(REPO)} "
        f"({total_lets} `let _ =`, {total_allows} `#[allow]`)."
    )


def main() -> int:
    counts = production_files()
    if "--update" in sys.argv:
        write_baseline(counts)
        return 0

    baseline = load_baseline()
    failures: list[str] = []

    for rel, (lets, allows) in sorted(counts.items()):
        base = baseline.get(rel)
        if base is None:
            if lets or allows:
                failures.append(
                    f"{rel}: new file with {lets} `let _ =` + {allows} "
                    f"`#[allow]` — resolve them, or run --update to baseline "
                    f"deliberate debt (and justify it in the diff)"
                )
            continue
        base_lets, base_allows = base
        if lets == 0 and allows == 0:
            failures.append(
                f"{rel}: now 0/0 — remove it from the baseline (debt "
                f"resolved 🎉)"
            )
            continue
        if lets > base_lets:
            failures.append(
                f"{rel}: `let _ =` grew to {lets} (baseline {base_lets}) — "
                f"handle the Result instead of discarding it"
            )
        if allows > base_allows:
            failures.append(
                f"{rel}: `#[allow]` grew to {allows} (baseline {base_allows}) "
                f"— fix the lint or scope the allow tighter; do not add more"
            )

    for rel in sorted(baseline):
        if rel not in counts:
            failures.append(
                f"{rel}: in the baseline but no longer a tracked production "
                f"file — remove the stale entry"
            )

    if failures:
        print(f"Suppression ratchet: {len(failures)} violation(s):\n")
        for v in failures:
            print(f"  ✗ {v}")
        return 1

    tl = sum(lets for lets, _ in counts.values())
    ta = sum(allows for _, allows in counts.values())
    print(
        f"Suppression ratchet: OK — {tl} `let _ =`, {ta} `#[allow]` across "
        f"{len(counts)} production files; none grew, no new ones."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
