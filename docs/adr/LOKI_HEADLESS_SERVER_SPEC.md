<!-- SPDX-License-Identifier: Apache-2.0 -->

# AppThere Loki — Headless Server Spec (Print & Conversion)

**Status:** Ratified (v1) — open decisions closed 2026-07-01
**Series:** AppThere Cloud, ADRs C021–C028
**Companion:** `LOKI_SERVER_COLLABORATION_SPEC.md` (C012–C020)
**Target edition:** Rust 2024

---

## 0. Scope

A headless Loki deployment for office environments: batch/on-demand **printing** and **file
conversion** with no display, no GPU, and deterministic output. It runs as a worker that
consumes jobs from the collaboration server's queue **or** as a standalone CLI/HTTP service for
customers who only want conversion and printing without collaboration.

### Goals

- Render Loki documents to print-ready output with no GPU and no window server.
- Deterministic, conformance-validated fidelity (byte-stable where the format allows).
- Convert between all formats Loki already imports/exports, plus PDF.
- Dispatch to office printers over standard IPP.
- Horizontally scalable, stateless workers.
- Honour the confidentiality tiers from C014–C015.

### Non-goals (v1)

- Interactive preview (this is headless by definition).
- Server-side processing of Tier-2 (zero-knowledge) documents (impossible; see C026).
- OCR / scanning (out of scope).

### Engineering standards

Inherited verbatim from the collaboration spec (300-line ceiling, no `unwrap()`/`expect()` in
library code, `#![forbid(unsafe_code)]` at crate roots, `thiserror`, SPDX line 1, `fl!()` for
user-visible strings, audit-first / implement-second).

---

## 1. Architecture

```
      ┌──────────────────────────────────────────────────────────────┐
      │  Job source                                                    │
      │   • apalis queue (from loki-server)   • CLI    • HTTP endpoint  │
      └───────────────┬────────────────────────────────────────────────┘
                      │  Job { Render | Print | Convert | Thumbnail | Export }
          ┌───────────▼────────────┐
          │   loki-headless (N≥1)   │  stateless, GPU-free
          │  ┌───────────────────┐  │
          │  │ loki layout+text  │  │  (existing PaginatedLayout, Parley)
          │  ├───────────────────┤  │
          │  │ vello_cpu render  │  │  deterministic, no wgpu/GPU
          │  ├───────────────────┤  │
          │  │ krilla → PDF      │  │  PDF / PDF-A / PDF-X profiles
          │  ├───────────────────┤  │
          │  │ import/export     │  │  DOCX/ODT/XLSX/PPTX/ODS/ODP/ODG
          │  └───────────────────┘  │
          └───────┬─────────────┬────┘
                  │             │
        ┌─────────▼───┐   ┌─────▼──────────────┐
        │ IPP / CUPS  │   │ object storage /    │
        │ office print│   │ return artifact     │
        └─────────────┘   └─────────────────────┘
```

Crates:

| Crate | Responsibility |
|---|---|
| `loki-headless` | Worker binary + CLI + optional Axum HTTP endpoint. |
| `loki-render-cpu` | vello_cpu render path; deterministic rasterisation. |
| `loki-pdf` | krilla-based PDF/PDF-A/PDF-X emission. |
| `loki-print` | IPP client; printer discovery; job dispatch. |
| `loki-convert` | Format matrix orchestration over existing import/export. |
| (reuses) `loki-text`, `loki-doc-model`, `loki-fonts`, `appthere-conformance` |

---

## 2. ADRs

### ADR-C021 — Headless rendering via `vello_cpu` (GPU-free, deterministic)

**Context.** Office/server hardware frequently has no GPU, and print fidelity demands
determinism. Loki already chose `vello_cpu` for deterministic headless conformance rendering
(the `appthere-conformance` crate, ~141 cases via the promoted `loki-acid` harness).

**Decision.** The headless render path uses `vello_cpu` exclusively — the **same** path the
conformance harness exercises. No wgpu, no surface, no GPU dependency. This means the print
output and the conformance-tested output are produced by identical code, so print fidelity is
covered by the existing test suite rather than a parallel, untested path.

**Consequences.** Runs on any Linux box (CCX server, office NUC, container). Print regressions
are caught by the conformance harness. No display server required.

---

### ADR-C022 — PDF emission via `krilla`; PDF/A and PDF/X profiles

**Context.** The Cloud spec already chose `krilla` as the PDF backend. Office printing and
archival need conformance profiles, not just "a PDF".

**Decision.** `loki-pdf` emits via krilla with selectable profiles:

- **PDF 1.7** — default general output.
- **PDF/A-2b** — archival (records retention, GDPR-adjacent recordkeeping).
- **PDF/X-4** — print-production colour fidelity (ties into `appthere-color` ICC/CMYK).

Colour management routes through the published `appthere-color` crate (moxcms, CMYK, ICC, soft
proofing) so print colour matches Loki's on-screen soft-proof.

**Consequences.** One PDF engine serves screen, print, and archive. Archival and print-shop
outputs are first-class, not conversions-of-conversions.

---

### ADR-C023 — Printing via IPP dispatch of rendered PDF

**Context.** "Print documents in an office environment" means network printers, print queues,
and (often) CUPS. Rendering directly to a printer PDL per device is a fidelity and maintenance
trap.

**Decision.** Render → PDF (§C022) → dispatch over **IPP** (`ipp`/IPP-everywhere), optionally
through a CUPS server. `loki-print` supports:

- Printer discovery (IPP / DNS-SD) and explicit printer URIs.
- Job options: copies, duplex, media size, colour/mono, staple/finishing where advertised.
- Job status polling and completion reporting back to the audit log.

Direct-to-printer PDL (PCL/PostScript) is a documented later option, not v1.

**Consequences.** Works with essentially all modern office printers via one code path. The PDF
intermediate keeps fidelity identical to on-screen and archive output.

---

### ADR-C024 — Conversion pipeline reuses existing import/export, conformance-gated

**Context.** Loki already has full OOXML and ODF round-tripping (DOCX/ODT/XLSX/PPTX/ODS/ODP/ODG)
plus PDF export. Conversion is orchestration, not new format code.

**Decision.** `loki-convert` exposes a conversion matrix built entirely on the existing
import/export crates:

- Import source → in-memory `doc-model` → export target (or PDF via `loki-pdf`).
- Every supported pair is registered in a static capability table; unsupported pairs return a
  typed `ConversionUnsupported` error rather than a lossy best-effort.
- Fidelity of each pair is asserted by the `appthere-conformance` harness, extending the ACID
  test plan (`LOKI_ACID_TEST_PLAN.md`). The PPTX generator gap noted there (29 cases documented,
  file unbuilt) is a prerequisite before PPTX conversion is declared supported.

**Consequences.** No second, drifting implementation of format handling. Conversion quality is
exactly the round-trip quality already measured. New format support lands in one place.

---

### ADR-C025 — Job model via `apalis`; idempotency, retries, dead-letter

**Context.** The Cloud spec already chose `apalis` for job queuing (Postgres backend, so no
Redis dependency is forced).

**Decision.** Job types: `Render`, `Print`, `Convert`, `Thumbnail`, `Export`. Each carries:

- an **idempotency key** (duplicate submissions coalesce),
- a **document reference + version** (so a job renders a pinned snapshot, not a moving target),
- **retry policy** (bounded, exponential backoff) and a **dead-letter** queue for poison jobs.

Workers are stateless and horizontally scalable; concurrency is bounded per worker to protect
memory (Loki's steady-state is ~750 MB shared via `Arc<Renderer>`/`PaginatedLayout`/
`FontResources`; the worker reuses that sharing across jobs in-process).

**Consequences.** Print/convert scale by adding workers. Failures are visible and recoverable,
not silent. Batch office workloads (e.g. nightly statement runs) are natural.

---

### ADR-C026 — Headless workers operate only on decryptable documents (Tier 0/1)

**Context.** Per collaboration ADR-C015, a Tier-2 (zero-knowledge) document is ciphertext to the
server; a server-side worker cannot render it. This is a hard boundary.

**Decision.**

- Server-side headless jobs are accepted **only for Tier 0 and Tier 1** documents. Under Tier 1
  the worker obtains the document DEK from the customer KMS/HSM **within the trust boundary** to
  decrypt, render, then discard the key from memory.
- For **Tier 2**, the headless engine is still available but must run **client-side / in-boundary**
  with a **caller-supplied key**: the same `loki-headless` binary runs as a local process the
  client feeds the DEK to, so a Tier-2 user can still print/convert locally without the key ever
  reaching the shared server. The server-side queue rejects Tier-2 jobs with
  `E2eeCapabilityDisabled`.

**Consequences.** The confidentiality guarantee is never quietly broken. Tier-2 users keep
print/convert capability, relocated to where the key legitimately lives.

---

### ADR-C027 — Deterministic font resolution and substitution policy

**Context.** Print fidelity collapses if the server substitutes fonts differently from the
author's machine. Loki already bundles Gelasio (metric-compatible with Georgia) as a
substitution anchor.

**Decision.**

- `loki-fonts` performs **deterministic resolution**: bundled fonts first (Gelasio et al.), then
  a configured, ordered fallback set, then a **fail-closed** option for print jobs that must not
  substitute silently (returns `FontUnavailable` instead of guessing).
- The active substitution map is recorded in job metadata and the audit log, so a print run is
  reproducible and explainable.
- Metric-compatible substitutes (e.g. Gelasio→Georgia, and the standard Liberation/Carlito style
  pairings) are the documented defaults for office interchange.

**Consequences.** Same document → same pages → same output on any worker. Font drift becomes a
policy choice, not an accident.

---

### ADR-C028 — Confidential-computing hardening for Tier-1 workers (SEV-SNP / TDX)

**Context.** Under Tier 1, a headless worker holds document plaintext **and** the DEK in memory
while it renders. On a shared or provider-operated host, that in-use plaintext is exposed to a
compromised or curious hypervisor / host operator — the one gap that at-rest and in-transit
encryption do not close. C5:2026 introduces explicit confidential-computing criteria.

**Decision (adopted).** Tier-1 headless workers run inside a hardware Trusted Execution
Environment — **AMD SEV-SNP** (or equivalently **Intel TDX**) — which encrypts VM memory and CPU
register state per-VM and attests the running image. Key release is **attestation-gated**: the
customer KMS/HSM unwraps the Tier-1 KEK only for a worker presenting a valid remote-attestation
report (via `snpguest`/`snphost`) proving genuine AMD hardware, a verified TCB/firmware level,
and the expected `loki-headless` measurement. A worker that cannot attest never receives a key
and therefore cannot decrypt.

**Hetzner reality check.** Hetzner **Cloud** VMs do **not** support confidential computing — no
vTPM, no data-in-use encryption, no UEFI secure boot. SEV-SNP therefore requires one of:

- a **Hetzner dedicated / bare-metal AMD EPYC server** (3rd-gen "Milan" or newer) with
  self-managed host enablement (QEMU + OVMF; e.g. an Ubuntu 25.04-class host), **or**
- an EU sovereign cloud that exposes confidential VMs.

Azure/GCP confidential VMs exist but are **non-sovereign** for EU purposes and are noted only as
non-default fallbacks. The sovereign path is Hetzner dedicated EPYC bare metal.

**Consequences.** Closes the in-use exposure gap: even the infrastructure operator cannot read
Tier-1 plaintext while it is being processed — the confidential-computing control C5:2026 now
expects. It is an **optional deployment mode** — Tier-1 without a TEE still works (trusting the
host); Tier-1 with a TEE removes that trust. Non-attesting hosts are simply ineligible to
process Tier-1 jobs.

---

## 3. Interfaces

**CLI (standalone, no server required):**

```
loki-headless convert  --in report.docx --out report.pdf --profile pdf-a2b
loki-headless convert  --in sheet.xlsx   --out sheet.ods
loki-headless print    --in report.docx  --printer ipp://officeprinter.local/ipp/print \
                        --duplex --copies 3 --media A4
loki-headless render   --in doc.odt       --out doc.pdf --profile pdf-x4
```

**HTTP (optional, for in-house automation):**

```
POST /v1/convert   { source, target_format, profile }      -> artifact | 409 (Tier 2)
POST /v1/print     { source, printer_uri, options }         -> job id
GET  /v1/jobs/{id}                                          -> status
```

**Worker:** subscribes to the apalis queue from `loki-server`; same job types as HTTP.

All errors typed; the canonical rejections are `ConversionUnsupported`, `FontUnavailable`,
and `E2eeCapabilityDisabled`.

---

## 4. Deployment

- **Alongside `loki-server`** (Hetzner OpenTofu module / Helm chart): a worker pool of CCX
  (dedicated-vCPU) instances; no GPU nodes needed. Scale by replica count.
- **Confidential-computing pool (Tier-1 hardening, per ADR-C028):** a separate worker pool on
  SEV-SNP/TDX nodes — Hetzner dedicated EPYC bare metal (Milan+) or an EU confidential cloud —
  with attestation-gated key release. Tier-1 jobs needing in-use protection route here; all
  other jobs run on the standard CCX pool.
- **Standalone office box:** single container or binary + a CUPS/IPP printer on the LAN; no
  Postgres required for pure CLI conversion/print. This is the "office printing" deployment.
- **Air-gapped:** fully offline — bundled fonts, local ICC profiles, no network egress; suits
  the strictest sovereignty and classified-adjacent environments.

Deterministic, GPU-free, and self-contained means the same binary serves a Hetzner worker pool
and a single office NUC by the print room.

---

## 5. Ratified decisions

1. **PPTX conversion gating — RATIFIED.** PPTX conversion stays behind the unbuilt PPTX
   generator from `LOKI_ACID_TEST_PLAN.md` until its 29 cases pass.
2. **Fail-closed fonts — RATIFIED.** Fail-closed font substitution is the default for `print`
   jobs; best-effort for `convert`.
3. **CUPS dependency — RATIFIED.** Direct IPP is the default; CUPS is an optional backend.
4. **Confidential-computing workers — ADOPTED.** Tier-1 workers run in a SEV-SNP/TDX TEE with
   attestation-gated key release (ADR-C028). Optional deployment mode; sovereign path is Hetzner
   dedicated EPYC bare metal.
