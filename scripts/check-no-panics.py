#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Panic-guard gate (Spec 01 §6.1 / A-6).

Fails if a first-party library source file (`<crate>/src/**.rs`) contains a
forbidden panicking macro outside of test code:

    panic!   todo!   unimplemented!

These either abort the process on an unexpected path (`panic!`) or mark
unfinished work that must not ship (`todo!` / `unimplemented!`). Library code
must surface failures through its typed `thiserror` error enums instead.

**`unreachable!` is intentionally permitted.** It asserts an invariant the type
system can't express (e.g. "Reflow mode always returns Canvas") and each use in
the tree carries a justifying message — this is documentation, not an ad-hoc
abort. If you reach for `unreachable!`, give it a message.

Scope: first-party `<crate>/src/` only. Excluded: `#[cfg(test)]` modules,
`tests/`, `benches/`, `examples/`, the vendored `patches/*` tree, and any path
in `scripts/no-panics-exceptions.txt` (each needing a justification comment).

Usage:
    scripts/check-no-panics.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
EXCEPTIONS_FILE = REPO / "scripts" / "no-panics-exceptions.txt"
FORBIDDEN = re.compile(r"\b(panic!|todo!|unimplemented!)\s*[\(\[\{]")


def lib_src_files() -> list[Path]:
    out = subprocess.check_output(["git", "ls-files", "*.rs"], cwd=REPO, text=True)
    files = []
    for rel in out.splitlines():
        if not rel or rel.startswith("patches/"):
            continue
        if "/src/" not in rel:
            continue
        if (
            "/tests/" in rel
            or "/benches/" in rel
            or "/examples/" in rel
            or rel.endswith("_tests.rs")
            or rel.endswith("/tests.rs")
        ):
            continue
        files.append(REPO / rel)
    return files


def strip_test_modules(src: str) -> list[tuple[int, str]]:
    """Drop `#[cfg(test)]`-gated blocks via brace tracking."""
    lines = src.split("\n")
    out: list[tuple[int, str]] = []
    k, n = 0, len(lines)
    while k < n:
        if re.search(r"#\[cfg\(test\)\]", lines[k]):
            m, depth, started = k, 0, False
            while m < n:
                depth += lines[m].count("{") - lines[m].count("}")
                if "{" in lines[m]:
                    started = True
                if started and depth <= 0:
                    break
                m += 1
            k = m + 1
            continue
        out.append((k + 1, lines[k]))
        k += 1
    return out


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
    exceptions = load_exceptions()
    failures: list[str] = []
    checked = 0
    for f in lib_src_files():
        rel = str(f.relative_to(REPO))
        if rel in exceptions:
            continue
        checked += 1
        for ln, line in strip_test_modules(
            f.read_text(encoding="utf-8", errors="replace")
        ):
            if line.lstrip().startswith("//"):
                continue  # comment / doc-comment
            if FORBIDDEN.search(line):
                failures.append(f"{rel}:{ln}: {line.strip()[:100]}")

    if failures:
        print(f"Panic-guard gate: {len(failures)} violation(s) "
              f"({checked} lib files checked):\n")
        for v in failures:
            print(f"  ✗ {v}")
        print("\npanic!/todo!/unimplemented! are forbidden in library src. "
              "Return a typed thiserror error instead; for an invariant the type "
              "system can't express, use unreachable!(\"why\").")
        return 1

    print(f"Panic-guard gate: OK — {checked} lib files, no "
          f"panic!/todo!/unimplemented!.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
