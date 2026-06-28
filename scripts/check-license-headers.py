#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""License-header gate (Spec 01 §6.2 `spdx_header_line_one`, license-aware).

Every tracked `.rs` file in a first-party workspace crate must begin, on
**line 1**, with the SPDX identifier matching that crate's declared
`license` in its `Cargo.toml`. This enforces two things at once:

  * the SPDX-on-line-1 convention (CLAUDE.md), and
  * the per-crate license distinction — notably `loki-opc`, which is
    **MIT** (it will be released standalone) while the rest of the suite is
    **Apache-2.0**. See `docs/adr/0010-per-crate-licensing.md`.

Scope: first-party workspace members only. The vendored `patches/*` tree is
excluded (upstream code keeps its own `MIT OR Apache-2.0` headers). Files
listed in `scripts/license-header-exceptions.txt` (one repo-relative path per
line, `#` comments allowed) are skipped with a reason.

Usage:
    scripts/check-license-headers.py            # fail on any violation
    scripts/check-license-headers.py --list     # print the resolved crate→license map
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
EXCEPTIONS_FILE = REPO / "scripts" / "license-header-exceptions.txt"
LICENSE_RE = re.compile(r'^license\s*=\s*"([^"]*)"', re.MULTILINE)
PACKAGE_RE = re.compile(r'^\[package\]', re.MULTILINE)


def tracked_rs_files() -> list[Path]:
    out = subprocess.check_output(
        ["git", "ls-files", "*.rs"], cwd=REPO, text=True
    )
    return [REPO / line for line in out.splitlines() if line]


def crate_license_map() -> dict[Path, str]:
    """Map each first-party crate directory to its declared license."""
    out = subprocess.check_output(
        ["git", "ls-files", "*/Cargo.toml", "Cargo.toml"], cwd=REPO, text=True
    )
    crates: dict[Path, str] = {}
    for rel in out.splitlines():
        if not rel or rel.startswith("patches/"):
            continue
        manifest = REPO / rel
        text = manifest.read_text(encoding="utf-8", errors="replace")
        if not PACKAGE_RE.search(text):
            continue  # virtual/workspace-only manifest (e.g. root)
        m = LICENSE_RE.search(text)
        if m:
            crates[manifest.parent] = m.group(1)
    return crates


def owning_license(path: Path, crates: dict[Path, str]) -> tuple[Path, str] | None:
    """Nearest ancestor crate dir + its license, or None if not first-party."""
    best: tuple[Path, str] | None = None
    for crate_dir, lic in crates.items():
        if path.is_relative_to(crate_dir):
            if best is None or len(crate_dir.parts) > len(best[0].parts):
                best = (crate_dir, lic)
    return best


def load_exceptions() -> set[str]:
    if not EXCEPTIONS_FILE.exists():
        return set()
    out = set()
    for line in EXCEPTIONS_FILE.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            out.add(line)
    return out


def main() -> int:
    crates = crate_license_map()
    if "--list" in sys.argv:
        for d in sorted(crates):
            print(f"{d.relative_to(REPO)} -> {crates[d]}")
        return 0

    exceptions = load_exceptions()
    failures: list[str] = []
    checked = 0

    for f in tracked_rs_files():
        rel = str(f.relative_to(REPO))
        if rel in exceptions:
            continue
        owner = owning_license(f, crates)
        if owner is None:
            continue  # not part of a first-party crate (e.g. stray root file)
        _, lic = owner
        expected = f"// SPDX-License-Identifier: {lic}"
        try:
            first = f.read_text(encoding="utf-8", errors="replace").splitlines()[:1]
        except OSError as exc:
            failures.append(f"{rel}: cannot read ({exc})")
            continue
        checked += 1
        line1 = first[0] if first else ""
        if line1 != expected:
            failures.append(f"{rel}: line 1 is {line1!r}, expected {expected!r}")

    if failures:
        print(f"License-header gate: {len(failures)} violation(s) "
              f"({checked} files checked):\n")
        for v in failures:
            print(f"  ✗ {v}")
        print("\nFix: ensure line 1 is the SPDX id matching the crate's "
              "Cargo.toml `license`.\n"
              "loki-opc is MIT; the rest of the suite is Apache-2.0 "
              "(docs/adr/0010-per-crate-licensing.md).")
        return 1

    print(f"License-header gate: OK — {checked} files, all line-1 SPDX ids "
          f"match their crate license.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
