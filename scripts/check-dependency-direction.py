#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Dependency-direction gate (Spec 01 audit A-13; ADR-0009 invariant I1).

Every internal (workspace-member) **normal** dependency must point *downhill or
sideways* in the layer model from `docs/adr/0009-target-architecture.md`:
`layer(crate) >= layer(dep)`. An uphill edge — e.g. render → ui, or model →
layout — fails the build, which is what keeps the architecture from eroding
after Spec 01 lands.

A crate that is neither in `LAYERS` nor `EXEMPT` fails, forcing an explicit
layer assignment when a crate is added (ADR-0009: "the gate fails on any
unmapped crate"). `dev`/`build` dependency edges are ignored (only the shipped
dependency graph defines the architecture).

Usage:
    scripts/check-dependency-direction.py
    scripts/check-dependency-direction.py --graph   # print the internal edges
"""

from __future__ import annotations

import json
import subprocess
import sys

# Layer numbers — see ADR-0009 §"layer assignment table". Lower = more
# foundational. `loki-pdf` is the L3b "exporter-above-layout" tier (3.5).
LAYERS: dict[str, float] = {
    # L0 foundation
    "loki-primitives": 0, "loki-fonts": 0, "loki-i18n": 0, "loki-spell": 0,
    "loki-graphics": 0,
    # L1 model
    "loki-doc-model": 1, "loki-sheet-model": 1, "loki-presentation-model": 1,
    # L1 macro interpreter core (leaf: no internal deps; macro spec Phase 2)
    "loki-basic": 1,
    # L2 io/serde + content
    "loki-opc": 2, "loki-odf": 2, "loki-ooxml": 2, "loki-epub": 2,
    "loki-templates": 2,
    # L3 layout
    "loki-layout": 3,
    # L3b exporter-above-layout
    "loki-pdf": 3.5,
    # L4 render
    "loki-render-cache": 4, "appthere-canvas": 4, "loki-vello": 4,
    "loki-renderer": 4, "loki-render-cpu": 4,
    # L5 ui / app-shell
    "appthere-ui": 5, "loki-app-shell": 5,
    # L6 app binaries
    "loki-text": 6, "loki-spreadsheet": 6, "loki-presentation": 6,
    # ── Server subsystem (backend; web-server spec ADRs C012–C028) ──────────
    # A separate top-level stack: it consumes the document/format libraries
    # (loki-convert reaches down to loki-pdf at L3.5) but is never consumed by
    # the client app binaries, so it sits above L6. Layers order the server
    # crates by their own dependency graph (all edges verified downhill).
    # L7 server foundation
    "loki-model": 7, "loki-crypto": 7, "loki-server-audit": 7, "loki-print": 7,
    # L8 server services + document conversion
    "loki-server-store": 8, "loki-server-auth": 8, "loki-convert": 8,
    # L9 collaboration relay
    "loki-server-collab": 9,
    # L10 server API surface
    "loki-server-api": 10,
    # L11 server + headless binaries
    "loki-server": 11, "loki-headless": 11,
}

# Dev/test members outside the layering (ADR-0009): conformance/test/benchmark
# harnesses depend across every layer by design. Their outgoing edges are not
# checked.
EXEMPT: set[str] = {"loki-acid", "appthere-conformance", "loki-bench"}


def metadata() -> dict:
    out = subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        text=True,
    )
    return json.loads(out)


def main() -> int:
    meta = metadata()
    internal = {p["name"] for p in meta["packages"]}
    edges: list[tuple[str, str]] = []
    failures: list[str] = []
    unmapped: set[str] = set()

    for pkg in meta["packages"]:
        name = pkg["name"]
        if name not in LAYERS and name not in EXEMPT:
            unmapped.add(name)
        if name in EXEMPT:
            continue
        for dep in pkg["dependencies"]:
            dname = dep["name"]
            if dname not in internal or dname == name:
                continue
            if (dep.get("kind") or "normal") != "normal":
                continue  # dev/build deps don't define the architecture
            edges.append((name, dname))
            if dname in EXEMPT:
                failures.append(
                    f"{name} -> {dname}: depends on a test-only crate"
                )
                continue
            if name in LAYERS and dname in LAYERS and LAYERS[name] < LAYERS[dname]:
                failures.append(
                    f"{name} (L{LAYERS[name]:g}) -> {dname} (L{LAYERS[dname]:g}): "
                    f"UPHILL edge — a lower layer must not import a higher one"
                )
            if dname not in LAYERS:
                unmapped.add(dname)

    if "--graph" in sys.argv:
        for a, b in sorted(edges):
            la = f"L{LAYERS.get(a, '?'):g}" if a in LAYERS else "?"
            lb = f"L{LAYERS.get(b, '?'):g}" if b in LAYERS else "?"
            print(f"{a} ({la}) -> {b} ({lb})")
        return 0

    for u in sorted(unmapped):
        failures.append(
            f"{u}: not assigned a layer — add it to LAYERS in this script and "
            f"to ADR-0009's table (or to EXEMPT if it is test-only)"
        )

    if failures:
        print(f"Dependency-direction gate: {len(failures)} violation(s):\n")
        for v in sorted(set(failures)):
            print(f"  ✗ {v}")
        print("\nDependencies must flow downhill per docs/adr/"
              "0009-target-architecture.md (audit A-13 / invariant I1).")
        return 1

    print(f"Dependency-direction gate: OK — {len(edges)} internal edges, "
          f"all downhill/sideways across {len(LAYERS)} mapped crates.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
