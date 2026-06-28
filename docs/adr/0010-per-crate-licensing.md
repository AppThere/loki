<!--
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0010: Per-crate licensing — `loki-opc` is MIT, the suite is Apache-2.0

**Status:** Accepted
**Date:** 2026-06-28
**Deciders:** AppThere engineering
**Relates to:** [`spec-01-audit-report.md`](spec-01-audit-report.md) finding A-3; Spec 01 §6.2 (`spdx_header_line_one`)

---

## Context

The Loki suite is licensed **Apache-2.0** — the root `LICENSE` is the Apache
2.0 text and every first-party crate declares `license = "Apache-2.0"` in its
`Cargo.toml`, with `// SPDX-License-Identifier: Apache-2.0` on line 1 of each
`.rs` file.

**`loki-opc` is the deliberate exception.** It implements the Open Packaging
Conventions (ISO/IEC 29500-2) container layer — a self-contained, format-neutral
concern with no Loki-specific dependencies. It is intended to be **released as a
standalone crate** (its `Cargo.toml` already carries
`repository = "https://github.com/appthere/loki-opc"`), and for the widest
possible downstream reuse it will be published under the **MIT** license, not
Apache-2.0. The manifest already reflects this (`license = "MIT"`), and the
source files already carry `// SPDX-License-Identifier: MIT`.

Two problems motivated this ADR:

1. **The distinction was undocumented.** Nothing in the docs or CLAUDE.md
   explained why one crate diverges; CLAUDE.md's convention text said, flatly,
   that every `.rs` file must begin with the *Apache-2.0* SPDX line — which is
   wrong for `loki-opc` and would mislead a contributor or a naïve gate into
   "correcting" MIT headers to Apache.
2. **The distinction was unenforced, and inconsistently applied.** The Spec 01
   audit's A-3 finding ("`loki-opc` missing SPDX headers") was a **false
   positive** — its scan matched only the literal `Apache-2.0` string, so the
   MIT headers read as absent. The real issues the corrected audit surfaced were
   smaller: `loki-opc` files placed the SPDX id on **line 2** (copyright on line
   1), violating the SPDX-on-line-1 convention and the ordering every Apache
   crate uses, and one test file (`tests/package_tests.rs`) genuinely had no
   header.

## Decision

1. **`loki-opc` is MIT-licensed; the rest of the AppThere/Loki workspace is
   Apache-2.0.** This is a per-crate property, declared authoritatively by each
   crate's `Cargo.toml` `license` field.
2. **Each `.rs` file's line-1 SPDX identifier must match its crate's declared
   license.** Uniform shape across the suite: line 1 is
   `// SPDX-License-Identifier: <license>`, line 2 is the copyright line. The
   `loki-opc` files were reordered to put MIT SPDX on line 1; `package_tests.rs`
   received the header.
3. **`loki-opc` ships its own MIT `LICENSE` file** (`loki-opc/LICENSE`) so the
   standalone release is self-contained; the root Apache `LICENSE` continues to
   govern the rest of the workspace.
4. **A license-aware gate enforces (2)** in CI:
   `scripts/check-license-headers.py` resolves each first-party `.rs` file to its
   owning crate, reads that crate's `Cargo.toml` `license`, and fails if line 1
   is not the matching SPDX id. The vendored `patches/*` tree (upstream
   `MIT OR Apache-2.0`) is out of scope; a reviewed
   `scripts/license-header-exceptions.txt` allow-list exists for any future
   exception.

This realises the `spdx_header_line_one` gate Spec 01 §6.2 reserved, generalised
from "Apache-2.0 on line 1" to "the *crate's* license on line 1" so the MIT/Apache
split is mechanically un-violable rather than convention-only.

## Consequences

**Positive**

- The MIT/Apache boundary is documented and enforced; a PR that adds an Apache
  header to a `loki-opc` file (or vice-versa), or omits a header, fails CI.
- `loki-opc` is publish-ready: manifest, per-file SPDX, and a crate-local MIT
  `LICENSE` all agree.
- The gate is future-proof: a new crate is checked against *its own* declared
  license with no gate change; adding a second MIT (or differently-licensed)
  crate "just works."
- Corrects the Spec 01 A-3 false positive with an honest, narrower finding.

**Negative / costs**

- Contributors must keep `Cargo.toml` `license` and the file headers in sync —
  but that is exactly what the gate checks, so drift surfaces immediately.
- A crate that legitimately needs a mixed/expression license (`MIT OR
  Apache-2.0`) would need the gate's single-line expectation extended; not
  needed today (only `patches/*`, which is excluded).

**Neutral**

- No code behaviour changes — header reordering is comment-only.

## Alternatives considered

- **Keep the whole workspace Apache-2.0 (relicense `loki-opc`).** Rejected:
  loses the permissive-license reach that motivates a standalone OPC crate.
- **Document the exception but don't enforce it.** Rejected per Spec 01's thesis
  (D2): unenforced conventions regress — the A-3 false positive and the line-2
  drift are exactly that regression in miniature.
- **A hardcoded crate→license map in the gate.** Rejected in favour of reading
  `Cargo.toml`, so the manifest stays the single source of truth and new crates
  need no gate edit.
