#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Unsafe-policy gate (Spec 01 audit A-7).

Every first-party crate root (`<crate>/src/lib.rs`, or `src/main.rs` for a
bin-only crate) must carry one of:

  * `#![forbid(unsafe_code)]` — the default; the crate has no `unsafe`; or
  * `#![deny(unsafe_code)]`   — the crate needs `unsafe`, confined to a
                                narrowly-scoped `#[allow(unsafe_code)]` item,
                                AND the crate is on the reviewed allow-list
                                (`scripts/unsafe-policy-allowlist.txt`).

This makes the unsafe surface explicit and reviewed: a crate cannot silently
introduce `unsafe` (it would need either `forbid` — which rejects it — or an
allow-list entry). The allow-list and the set of `deny` crates must agree, so a
crate that stops needing unsafe can't leave a stale grant behind.

Scope: first-party workspace crates; the vendored `patches/*` tree is excluded.

Usage:
    scripts/check-unsafe-policy.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ALLOWLIST_FILE = REPO / "scripts" / "unsafe-policy-allowlist.txt"
FORBID = re.compile(r"#!\[forbid\(unsafe_code\)\]")
DENY = re.compile(r"#!\[deny\(unsafe_code\)\]")
NAME = re.compile(r'^\s*name\s*=\s*"([^"]+)"', re.MULTILINE)
PACKAGE = re.compile(r"^\[package\]", re.MULTILINE)


def first_party_crates() -> dict[str, Path]:
    """Map crate name -> crate dir for every first-party package."""
    out = subprocess.check_output(
        ["git", "ls-files", "*/Cargo.toml", "Cargo.toml"], cwd=REPO, text=True
    )
    crates: dict[str, Path] = {}
    for rel in out.splitlines():
        if not rel or rel.startswith("patches/"):
            continue
        manifest = REPO / rel
        text = manifest.read_text(encoding="utf-8", errors="replace")
        if not PACKAGE.search(text):
            continue
        m = NAME.search(text)
        if m:
            crates[m.group(1)] = manifest.parent
    return crates


def crate_root(crate_dir: Path) -> Path | None:
    for cand in ("src/lib.rs", "src/main.rs"):
        p = crate_dir / cand
        if p.exists():
            return p
    return None


def load_allowlist() -> set[str]:
    if not ALLOWLIST_FILE.exists():
        return set()
    out = set()
    for line in ALLOWLIST_FILE.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            out.add(line)
    return out


def main() -> int:
    crates = first_party_crates()
    allow = load_allowlist()
    failures: list[str] = []
    deny_crates: set[str] = set()

    for name, cdir in sorted(crates.items()):
        root = crate_root(cdir)
        if root is None:
            failures.append(f"{name}: no src/lib.rs or src/main.rs found")
            continue
        head = root.read_text(encoding="utf-8", errors="replace")
        has_forbid = bool(FORBID.search(head))
        has_deny = bool(DENY.search(head))
        rel = root.relative_to(REPO)
        if has_forbid:
            if name in allow:
                failures.append(
                    f"{name}: on the unsafe allow-list but root is "
                    f"`forbid(unsafe_code)` — remove the stale allow-list entry "
                    f"({rel})"
                )
            continue
        if has_deny:
            deny_crates.add(name)
            if name not in allow:
                failures.append(
                    f"{name}: `deny(unsafe_code)` (needs unsafe) but NOT on the "
                    f"allow-list — add it with justification or use `forbid` ({rel})"
                )
            continue
        failures.append(
            f"{name}: root has neither `#![forbid(unsafe_code)]` nor "
            f"`#![deny(unsafe_code)]` ({rel})"
        )

    # Allow-list entries that aren't first-party crates at all. (Real crates that
    # are listed-but-forbid are already reported as stale in the main loop.)
    for stale in sorted(allow - set(crates)):
        failures.append(f"allow-list: '{stale}' is not a first-party crate")

    if failures:
        print(f"Unsafe-policy gate: {len(failures)} violation(s) "
              f"({len(crates)} crates):\n")
        for v in failures:
            print(f"  ✗ {v}")
        print("\nEvery crate root must be `#![forbid(unsafe_code)]`, or "
              "`#![deny(unsafe_code)]` with an entry in "
              "scripts/unsafe-policy-allowlist.txt (Spec 01 audit A-7).")
        return 1

    print(f"Unsafe-policy gate: OK — {len(crates)} crates; "
          f"{len(deny_crates)} allow-listed deny ({', '.join(sorted(deny_crates)) or 'none'}), "
          f"rest forbid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
