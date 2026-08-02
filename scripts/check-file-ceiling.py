#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""File-ceiling gate (Spec 01 audit A-2 / CLAUDE.md 300-line ceiling).

No first-party production `.rs` file may exceed 300 lines. Pre-existing
violations are frozen in `scripts/file-ceiling-baseline.txt` as a **ratchet**:

  * a file NOT in the baseline must be <= 300 lines;
  * a baselined file may exceed 300 but must NOT grow beyond its recorded count
    (debt can only shrink);
  * when a baselined file drops to <= 300 it must be removed from the baseline
    (no stale grants);
  * a baseline entry for a missing/renamed file fails (keeps the list honest);
  * `CLAUDE.md`'s convenience table must list exactly the baselined paths.

So new files can't be born over-ceiling, the known over-ceiling files can't
grow, and the backlog monotonically shrinks toward empty.

The `CLAUDE.md` check exists because that copy drifted: it claimed 29 entries and
seven paths that had all been split, and `CLAUDE.md` is the file that loads
automatically, so the wrong number was the one every session read first. Telling
the reader "believe the baseline" is the documented fix and would have drifted
again — this makes it unavailable instead (L08-043). Paths only, not counts: the
counts are the ratchet's business and would fail this gate on every split.

Scope: first-party production `.rs`. Excluded (CLAUDE.md exempts test files):
`tests/`, `*_tests.rs`, `*/tests.rs`, `benches/`, `examples/`, and `patches/*`.

Usage:
    scripts/check-file-ceiling.py            # enforce
    scripts/check-file-ceiling.py --update   # rewrite the baseline from current
                                             # counts (review the diff!)
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BASELINE_FILE = REPO / "scripts" / "file-ceiling-baseline.txt"
CLAUDE_MD = REPO / "CLAUDE.md"
CEILING = 300


def is_test(rel: str) -> bool:
    return (
        "/tests/" in rel
        or rel.endswith("_tests.rs")
        or rel.endswith("/tests.rs")
        or "/benches/" in rel
        or "/examples/" in rel
        or rel.startswith("patches/")
    )


def production_files() -> dict[str, int]:
    out = subprocess.check_output(["git", "ls-files", "*.rs"], cwd=REPO, text=True)
    counts: dict[str, int] = {}
    for rel in out.splitlines():
        if not rel or is_test(rel):
            continue
        text = (REPO / rel).read_text(encoding="utf-8", errors="replace")
        # Number of lines, matching `wc -l` (trailing newline not double-counted).
        counts[rel] = text.count("\n") + (0 if text.endswith("\n") or not text else 1)
    return counts


def load_baseline() -> dict[str, int]:
    base: dict[str, int] = {}
    if not BASELINE_FILE.exists():
        return base
    for line in BASELINE_FILE.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        count, path = line.split(None, 1)
        base[path.strip()] = int(count)
    return base


def write_baseline(counts: dict[str, int]) -> None:
    over = sorted(
        ((n, p) for p, n in counts.items() if n > CEILING), reverse=True
    )
    lines = [
        "# File-ceiling baseline — pre-existing >300-line production files (A-2).",
        "# Format: `<linecount> <path>`. Ratcheted by scripts/check-file-ceiling.py:",
        "# these may not GROW, and must be removed once split to <= 300 lines.",
        "# Regenerate with: scripts/check-file-ceiling.py --update",
        "",
    ]
    lines += [f"{n} {p}" for n, p in over]
    BASELINE_FILE.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Wrote {len(over)} entries to {BASELINE_FILE.relative_to(REPO)}")


def claude_md_drift(baseline: dict[str, int]) -> list[str]:
    """Paths the `CLAUDE.md` backlog table and the baseline disagree about.

    Membership only. A path in one and not the other is drift; the *numbers* in
    that table are prose and go stale between splits by design, so checking them
    would fail the gate on every split and train everyone to run `--update`
    without reading it.
    """
    try:
        text = CLAUDE_MD.read_text(encoding="utf-8")
    except OSError:
        return ["CLAUDE.md is unreadable — cannot check the backlog table"]

    # The table's own rows, and only those. Scoping matters in BOTH directions:
    # the split-technique narrative above the table names most of the files ever
    # split, so a whole-file substring search finds a path that the table itself
    # dropped. The first draft did exactly that, and a mutation removing a row
    # passed the gate — the half of the check that reports "you are understating
    # the debt" was dead, which is the half that matters.
    listed = set()
    for line in text.splitlines():
        if not line.startswith("| `") or "|---" in line:
            continue
        rel = line.split("`")[1]
        if rel.endswith(".rs"):
            listed.add(rel)

    out = []
    for rel in sorted(set(baseline) - listed):
        out.append(
            f"{rel}: baselined but absent from CLAUDE.md's backlog table — "
            f"add it, or the auto-loading file understates the debt"
        )
    for rel in sorted(listed - set(baseline)):
        out.append(
            f"{rel}: named in CLAUDE.md's backlog table but not baselined "
            f"— it was split; remove the row"
        )
    return out


def main() -> int:
    counts = production_files()
    if "--update" in sys.argv:
        write_baseline(counts)
        return 0

    baseline = load_baseline()
    failures: list[str] = []

    for rel, n in sorted(counts.items()):
        if rel in baseline:
            if n <= CEILING:
                failures.append(
                    f"{rel}: now {n} lines (<= {CEILING}) — remove it from "
                    f"the baseline (debt resolved 🎉)"
                )
            elif n > baseline[rel]:
                failures.append(
                    f"{rel}: grew to {n} lines (baseline {baseline[rel]}) — "
                    f"split it; do not grow over-ceiling files"
                )
        elif n > CEILING:
            failures.append(
                f"{rel}: {n} lines > {CEILING} ceiling — split it (or, for "
                f"genuine pre-existing debt, run --update to baseline it)"
            )

    for rel in sorted(baseline):
        if rel not in counts:
            failures.append(
                f"{rel}: in the baseline but no longer a tracked production file "
                f"— remove the stale entry"
            )

    failures.extend(claude_md_drift(baseline))

    if failures:
        print(f"File-ceiling gate: {len(failures)} violation(s):\n")
        for v in failures:
            print(f"  ✗ {v}")
        return 1

    over = sum(1 for p, n in counts.items() if n > CEILING)
    print(f"File-ceiling gate: OK — {len(counts)} production files, "
          f"{over} baselined over-ceiling (ratcheted), none grew, no new ones.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
