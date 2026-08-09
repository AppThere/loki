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
  * `#[allow(…)]` / `#[expect(…)]` — a lint suppression. CLAUDE.md permits
    narrowly-scoped, justified allows, so we do not forbid them outright — we
    freeze the current population and require every *new* one to be a conscious
    `--update`. `expect` counts as well as `allow`: it is the same debt with an
    expiry, and counting only `allow` would leave a spelling that walks past
    this gate.

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

from gate_source import code_only

REPO = Path(__file__).resolve().parent.parent
BASELINE_FILE = REPO / "scripts" / "suppressions-baseline.txt"

# Largest single-commit fall in either total this gate will accept without being
# told to.
#
# # Why a downward guard on a ratchet whose whole point is downward
#
# Down was safe because down meant the debt went away. It does not always: when
# `gate_source` first stripped a bare `#` as a comment, every `#[allow(…)]` line
# became prose and this gate's allow count fell from 188 to **zero** — and it
# reported that as a hundred files' debt cheerfully resolved. **A gate reporting
# improvement is the one nobody double-checks.** It was caught by wanting a
# number for a commit message, which is not a control.
#
# 15% in one commit is either a real sweep — in which case you are running
# `--update` anyway and can say `--accept-collapse` — or a broken detector. Both
# deserve a human; only one of them deserves a green build.
MAX_COLLAPSE_FRACTION = 0.15

# `let _ = …` (optionally typed: `let _: T = …`). Anchored at a statement
# boundary so `slet _ =` / identifiers ending in "let" do not match.
LET_UNDERSCORE = re.compile(r"(?<![\w])let\s+_\s*(?::[^=]+)?=")
# `#[allow(` / `#![allow(`, and the same two spellings of `expect`.
#
# `#[expect]` counts because it is the same debt with a better expiry: it is a
# lint suppression that *fails the build* once the lint would no longer fire.
# Counting only `allow` left a silent way around this ratchet — write `expect`
# and the file reads as clean — which is the surface-that-permits shape
# (L08-043). Prefer `expect` where the suppression is meant to expire; it still
# has to go through `--update`.
ALLOW = re.compile(r"#!?\[(?:allow|expect)\(")


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
    # Comments and string literals stripped first: this gate's own historical
    # false positive was a *comment about* `let _ =` (Spec 08 §8), which is the
    # shape `gate_source` exists to remove once rather than per gate.
    code = code_only(text)
    return len(LET_UNDERSCORE.findall(code)), len(ALLOW.findall(code))


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
        "# Suppression baseline — pre-existing `let _ =` swallows + `#[allow]`/`#[expect]` "
        "(Q-3/Q-4).",
        "# Format: `<let_underscore_count> <allow_or_expect_count> <path>`. Ratcheted by",
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


def collapse_report(
    baseline: dict[str, tuple[int, int]], counts: dict[str, tuple[int, int]]
) -> str | None:
    """A message when either total fell implausibly far, else `None`."""
    base_l = sum(l for l, _ in baseline.values())
    base_a = sum(a for _, a in baseline.values())
    now_l = sum(l for l, _ in counts.values())
    now_a = sum(a for _, a in counts.values())
    for name, was, now in (("`let _ =`", base_l, now_l), ("allow/expect", base_a, now_a)):
        if was > 0 and now < was * (1.0 - MAX_COLLAPSE_FRACTION):
            fell = (was - now) * 100 // was
            return (
                f"{name} fell {fell}% in one step ({was} to {now}), past the "
                f"{int(MAX_COLLAPSE_FRACTION * 100)}% this gate accepts unasked. "
                f"That is either a real sweep or a **broken detector** — the "
                f"second is what happened the one time it fired, and it looked "
                f"exactly like the first. If the debt genuinely went away, run "
                f"`--update --accept-collapse`."
            )
    return None


def main() -> int:
    # The report uses a non-ASCII glyph (✗); force UTF-8 output so a violation
    # message does not itself crash on a cp1252 console (Windows-local runs).
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    counts = production_files()
    baseline = load_baseline()
    collapse = collapse_report(baseline, counts)

    if "--update" in sys.argv:
        if collapse and "--accept-collapse" not in sys.argv:
            print(f"Refusing to rewrite the baseline:\n\n  ✗ {collapse}")
            return 1
        write_baseline(counts)
        return 0

    failures: list[str] = []
    if collapse:
        failures.append(collapse)

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
