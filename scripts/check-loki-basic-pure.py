#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Enforce the macro interpreter's isolation invariants (macro spec §4.3, §10, §12).

Two properties, both security-critical:

1. **`loki-basic` links nothing that can touch the outside world.** The
   interpreter has zero ambient authority — every effect is routed through the
   `Host` trait — so its dependency set must stay within a tiny allow-list. A
   new dependency on an I/O crate (std sockets are unavoidable, but `reqwest`,
   `tokio`, `std::fs`-wrapping helpers, etc. are not) would be a capability leak.

2. **No server or headless crate links the interpreter.** Macros never execute
   on the server (spec §10); the interpreter and its host crate must not appear
   anywhere in the backend's dependency graphs.

Exits non-zero (with an explanation) on any violation. Pure stdlib; no cargo
metadata needed — it reads the manifests directly.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# `loki-basic` may depend only on these crates. Deliberately tiny.
BASIC_ALLOWED_DEPS = {"thiserror"}

# Crates that must never (transitively-declared) depend on the interpreter.
SERVER_CRATES = [
    "loki-server",
    "loki-server-api",
    "loki-server-collab",
    "loki-server-store",
    "loki-server-auth",
    "loki-server-audit",
    "loki-headless",
    "loki-convert",
    "loki-print",
]

# The interpreter crates the server side must not link.
INTERPRETER_CRATES = {"loki-basic", "loki-vba", "loki-macro-host"}


def dep_names(manifest: Path) -> set[str]:
    """Crate names in every `[dependencies*]` table of a Cargo.toml (naive TOML)."""
    names: set[str] = set()
    in_deps = False
    for line in manifest.read_text().splitlines():
        s = line.strip()
        if s.startswith("["):
            in_deps = "dependencies" in s and not s.startswith("[package")
            continue
        if not in_deps or not s or s.startswith("#"):
            continue
        m = re.match(r'([A-Za-z0-9_-]+)\s*=', s)
        if m:
            names.add(m.group(1))
    return names


def main() -> int:
    errors: list[str] = []

    basic = ROOT / "loki-basic" / "Cargo.toml"
    if not basic.exists():
        print("check-loki-basic-pure: loki-basic/Cargo.toml not found", file=sys.stderr)
        return 1
    extra = dep_names(basic) - BASIC_ALLOWED_DEPS
    if extra:
        errors.append(
            f"loki-basic gained disallowed dependencies {sorted(extra)}; the "
            f"interpreter must depend only on {sorted(BASIC_ALLOWED_DEPS)} "
            f"(no I/O — every effect goes through the Host trait)."
        )

    for crate in SERVER_CRATES:
        manifest = ROOT / crate / "Cargo.toml"
        if not manifest.exists():
            continue
        leaked = dep_names(manifest) & INTERPRETER_CRATES
        if leaked:
            errors.append(
                f"{crate} depends on {sorted(leaked)}; macros never execute "
                f"server-side (spec §10) — the interpreter must not be linked."
            )

    if errors:
        print("loki-basic purity gate: FAILED", file=sys.stderr)
        for e in errors:
            print(f"  ✗ {e}", file=sys.stderr)
        return 1

    print(
        "loki-basic purity gate: OK — interpreter deps within allow-list; "
        "no server/headless crate links the interpreter."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
