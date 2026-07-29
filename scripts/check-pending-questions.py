#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Pending-question trigger (Spec 08 L9-019's third clause, made mechanical).

Every open finding in this program records **what would settle it**. That is a
register of questions keyed by the instrument that answers them — and it had no
trigger: nothing prompted a re-read when an instrument finally landed.

The cost of that gap is on the record. The macOS total-only memory probe shipped
in r53 with a documented rationale; the regime sweep that refuted the rationale
landed in r55; the two were connected in r56 by a person noticing, three commits
later. Both artefacts were in the tree the whole time.

So this gate watches `scripts/pending-questions.txt` for rows whose instrument
does not exist yet, and fails the moment one does. **The failure is not "answer
the question" — it is "re-read what you decided while it was open."** A new
counter is retroactive evidence about old decisions, and that is the half a
forward-looking discipline misses.

Resolving a row means doing the sweep and moving it to `landed` with the outcome
written in. Rows are never deleted.

# The blind spot, and its control

**This gate detects an instrument arriving where a row predicted it would.** If
one lands somewhere else — a different module, a different name, folded into an
existing file — the gate stays silent and the row sits in the register reading as
still-open while the question has quietly become answerable. That is the same
class as an instrument that cannot speak, one level up: a register that cannot
detect arrival.

The path prediction is the right *primary* trigger — it is specific, and it fires
at the moment that matters. What it needs is a control that does not depend on
the prediction being right, and the control is a fixed moment rather than a
cleverer pattern: **review the whole register at every phase close**, which is
already a ritual with a checklist. `--list` prints it for exactly that.

Usage:
    scripts/check-pending-questions.py
    scripts/check-pending-questions.py --list   # for the phase-close review
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REGISTER = REPO / "scripts" / "pending-questions.txt"

# The register itself names the instruments, so searching it would match every
# row trivially. Its own text is not evidence that anything was built.
SELF = "scripts/pending-questions.txt"


def rows() -> list[tuple[str, str, str, str, int]]:
    out: list[tuple[str, str, str, str, int]] = []
    if not REGISTER.exists():
        return out
    for num, line in enumerate(REGISTER.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 4:
            out.append(("malformed", line.strip(), "", "", num))
            continue
        out.append(
            (parts[0].strip(), parts[1].strip(), parts[2].strip(), parts[3].strip(), num)
        )
    return out


# A comment naming an instrument is not the instrument. The first draft of this
# gate matched anywhere and fired on its own TODO comments, an OOXML schema and a
# spike write-up — reporting three arrivals, none of them built. Prose that names
# what is missing is exactly what an open question *looks* like, so matching it is
# the failure mode rather than an edge case.
COMMENT = re.compile(r"^\s*(//|#|\*|<!--)")


def instrument_exists(glob: str, pattern: str) -> list[str]:
    """Files under `glob` where the instrument appears **in code**, not prose."""
    try:
        found = subprocess.run(
            ["git", "grep", "-nE", pattern, "--", glob],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return []
    hits = []
    for line in found.stdout.splitlines():
        parts = line.split(":", 2)
        if len(parts) < 3 or parts[0] == SELF:
            continue
        if COMMENT.match(parts[2]):
            continue
        hits.append(f"{parts[0]}:{parts[1]}")
    return hits


def main() -> int:
    if "--list" in sys.argv:
        # The phase-close control: every row, whatever the path prediction did.
        for state, glob, instrument, question, num in rows():
            print(f"[{state}] line {num}: {instrument}  (looking in {glob})")
            print(f"    {question}\n")
        return 0

    failures: list[str] = []
    awaiting = landed = 0

    for state, glob, instrument, question, num in rows():
        if state == "malformed":
            failures.append(
                f"line {num}: expected four tab-separated fields (state, "
                f"path glob, instrument, question), got: {glob[:60]}"
            )
            continue
        try:
            re.compile(instrument)
        except re.error as e:
            failures.append(f"line {num}: instrument is not a valid regex ({e})")
            continue

        hits = instrument_exists(glob, instrument)
        if state == "awaiting":
            awaiting += 1
            if hits:
                failures.append(
                    f"line {num}: the instrument `{instrument}` now exists in "
                    f"{glob} ({', '.join(hits[:3])}). Before using it going "
                    f"forward, "
                    f"re-read what was decided while this was open:\n"
                    f"      {question}\n"
                    f"    Then move the row to `landed` with the outcome written in."
                )
        elif state == "landed":
            landed += 1
            if not hits:
                failures.append(
                    f"line {num}: marked `landed` but the instrument "
                    f"`{instrument}` is not in {glob} — a resolved question "
                    f"whose evidence has been removed is an open question again"
                )
        else:
            failures.append(f"line {num}: unknown state `{state}`")

    if failures:
        print(f"Pending questions: {len(failures)} trigger(s):\n")
        for f in failures:
            print(f"  ✗ {f}")
        return 1

    print(
        f"Pending questions: OK — {awaiting} awaiting their instrument, "
        f"{landed} landed and swept."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
