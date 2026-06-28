<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 01 — Codebase Audit & Target Architecture

| | |
|---|---|
| **Status** | Draft — pending implementation |
| **Scope** | AppThere Loki (Text), with monorepo-wide enforcement primitives |
| **Sequence** | 1 of 6 — foundational; other specs conform to the architecture this one establishes |
| **Depends on** | Nothing |
| **Feeds** | Testing harness (gates), Responsive (kills layout hardcodes), Ribbon, Styling Panel, Benchmarking (shares CI infra) |

---

## 1. Context & Motivation

The majority of Loki-specific code is AI-generated. Much of it works, but not every decision was intentional or sound, and incidental shortcuts have begun to ossify into load-bearing assumptions. The canonical example: hit-detection in the Vello document renderer works around a hardcoded **1280px window width** that resurfaces in multiple places despite the app being nominally responsive. That number is a *temporary decision that became permanent* because nothing in the toolchain objects to it.

This spec does two things, in order:

1. **Audit** the entire codebase to surface incidental, inconsistent, or unsound decisions — magic numbers, leaked `unwrap()`s, hardcoded dimensions, dead code, inconsistent error handling, layering violations — and present them for triage rather than pre-judging severity.
2. **Establish enforcement** so that once a smell is fixed, the toolchain prevents its return. The goal is not a one-time sweep; it is making the conventions *mechanically un-violable* in CI.

A third, dependent output: the audit **proposes a clean target architecture** derived from what already exists and what is reasonably reachable from the current state — not an idealized rewrite. Every subsequent spec conforms to that architecture.

This is an **audit-first, implement-second** task. The implementing agent inspects the live codebase before proposing or changing anything; the file and crate references in this document are illustrative, not authoritative, and may be stale.

---

## 2. Goals / Non-Goals

**Goals**

- Produce a complete, triageable inventory of code smells and architectural inconsistencies.
- Propose a target layering and crate-dependency invariant grounded in the existing crate graph.
- Implement enforcement (lints + CI gates) for every convention Loki already claims to hold.
- Eliminate the specific class of bug the 1280px hardcode represents: layout/render logic that assumes a fixed viewport.

**Non-Goals**

- Rewrites. Fixes are intentional and incremental; the architecture is *reachable*, not aspirational.
- Behavioral feature work. This spec changes structure and removes incidental decisions; it does not add user-facing features.
- Performance optimization (owned by Spec 06 — Benchmarking) beyond removing obviously wasteful incidental code the audit surfaces.
- Fixing every smell in one pass. The audit *surfaces and triages*; fixes are scheduled against the ranked list.

---

## 3. Working Method (for the implementing agent)

Follow the established Fix Session discipline:

1. **Audit pass — read only.** Inspect every crate. Produce the inventory in §4. Change nothing.
2. **Triage.** Present the inventory as a ranked table (§4.3) for the maintainer to confirm/reorder priority. Do not assume severity ordering.
3. **Architecture proposal.** Produce §5 as a standalone ADR within this document's companion set, validated against the real dependency graph.
4. **Enforcement.** Implement §6 (lints + CI gates). These land *before* the bulk of fixes, so fixes are verified by the gates as they're made.
5. **Fixes.** Work the triaged list top-down, one ADR per non-trivial decision, each with tradeoffs recorded.

Standing standards assumed everywhere (flag any deviation found, do not silently conform):
300-line file ceiling · no `unwrap()`/`expect()` in library code · `#![forbid(unsafe_code)]` at crate roots (documented exceptions only) · typed errors via `thiserror` · Apache-2.0 SPDX header on line 1 · Rust 2024 edition.

---

## 4. The Audit

### 4.1 Smell categories to surface

The audit inspects for at least the following. It **surfaces and counts**; it does not pre-rank.

- **Hardcoded dimensions / magic numbers** — the 1280px class. Any numeric literal in a layout, hit-test, or render code path that is not a named constant or sourced from a viewport/config value. Flag clusters where the same magic value recurs (strong signal of a leaked assumption).
- **Leaked `unwrap()` / `expect()` / `panic!`** in library code, distinct from test-only usage (which is acceptable). Report path + whether reachable from a public API.
- **`unsafe` blocks** outside the documented exceptions (Android `NativeActivity` in `loki-text`/`loki-presentation`/`loki-spreadsheet`). Each must carry a `// SAFETY:` justification; flag any that don't.
- **File-ceiling violations** (>300 lines) with current line counts.
- **SPDX header issues** — missing, or not on line 1.
- **Inconsistent error handling** — ad-hoc `Box<dyn Error>`, `String` errors, or `Result` types not backed by a `thiserror` enum.
- **Dead / unreachable code** — unused `pub` surface, orphaned modules, behind-`cfg` code that never compiles for any target.
- **Duplication** — copy-pasted logic that should be a shared helper or crate (especially anything duplicated across the Text/Presentation/Spreadsheet members).
- **`HACK` / `TODO` / `FIXME` / `XXX` debt** — catalogued with surrounding context so each can be converted to a tracked decision or resolved.
- **Naming inconsistency** — divergent conventions for the same concept across crates (e.g. `*Props` vs `*Config` vs `*Options`).
- **Layering violations** — any dependency that points "uphill" against the target architecture (§5). This category can only be fully populated after §5 is drafted; the audit produces the raw dependency graph first.

### 4.2 The 1280px class — special handling

This pattern is the named motivation, so it gets a dedicated sub-investigation. The agent must:

1. Find **every** occurrence of a hardcoded viewport/page/window dimension across render, layout, and hit-test code.
2. Trace each to its origin and document what *should* supply the value (live viewport, page geometry, zoom state, DPI).
3. Propose the single source of truth that replaces them — a `Viewport`/`LayoutContext` type threaded through render and hit-test — so that the dylint in §6.2 has a sanctioned alternative to point authors toward.

### 4.3 Deliverable format

A markdown audit report committed alongside this spec, with a triage table the maintainer ranks:

| ID | Category | Location(s) | Count | Blast radius | Proposed fix | Priority (maintainer) |
|----|----------|-------------|-------|--------------|--------------|----------------------|

"Blast radius" = how much code/behavior a fix touches, so triage can weigh effort against risk. The **Priority** column is left blank for the maintainer to fill.

---

## 5. Target Architecture (proposed by the agent)

The agent proposes a layering derived from the *existing* crates, not an idealized one. The expected shape, to be confirmed and refined against reality:

```
                 ┌─────────────────────────────────────┐
   app binary    │  loki (wires everything)            │
                 └─────────────────────────────────────┘
                 ┌─────────────────────────────────────┐
   ui            │  loki ribbon / panels · appthere_ui │
                 └─────────────────────────────────────┘
                 ┌─────────────────────────────────────┐
   render        │  loki-renderer (Vello/wgpu) ·       │
                 │  appthere-canvas                    │
                 └─────────────────────────────────────┘
                 ┌─────────────────────────────────────┐
   layout        │  pagination · text layout (Parley)  │
                 └─────────────────────────────────────┘
   ── UDOM waist ── narrow, format-neutral boundary ──
                 ┌─────────────────────────────────────┐
   io / serde    │  loki-odf · loki-opc · ooxml ·      │
                 │  loro_bridge                        │
                 └─────────────────────────────────────┘
                 ┌─────────────────────────────────────┐
   model         │  loki-doc-model · loki-sheet-model  │
                 └─────────────────────────────────────┘
                 ┌─────────────────────────────────────┐
   foundation    │  loki-primitives · loki-fonts ·     │
                 │  appthere-color                     │
                 └─────────────────────────────────────┘
```

### 5.1 Invariants the audit validates

- **Acyclic, downhill-only dependencies.** No crate depends on a layer above it. `model` never imports `render`, `layout`, or `ui`. `render` never imports `ui`.
- **The UDOM waist.** Serialization (ODF/OOXML) talks to the model through a narrow, format-neutral document interface — the hourglass. Format-specific types do not leak upward into layout/render, and render types do not leak down into serialization.
- **Foundation purity.** `loki-primitives`, `loki-fonts`, `appthere-color` depend on nothing internal except possibly each other in a fixed order; they are the leaves.
- **`forbid(unsafe_code)` everywhere except the documented Android-`NativeActivity` crates**, and even there, scoped and `SAFETY`-justified.

### 5.2 What the architecture ADR must contain

For each proposed boundary: the rule, why it holds given current code, the violations the audit found against it, and the migration path to conformance. Where the current graph can't reach the target cheaply, the ADR says so and proposes the pragmatic intermediate state rather than forcing a costly move.

---

## 6. Enforcement

Conventions that aren't mechanically enforced regress. Each gate below runs in CI and fails the build on violation.

### 6.1 Clippy configuration

- `clippy.toml` `disallowed-methods` for `Result::unwrap`, `Result::expect`, `Option::unwrap`, `Option::expect`, `panic!` **in library targets** (tests exempt via target config).
- Workspace-level `#![deny(clippy::all, clippy::pedantic)]` with a curated, documented allow-list rather than blanket `allow`.

### 6.2 Custom dylint lints

- **`no_hardcoded_layout_dims`** — flags numeric literals above a small threshold appearing in render/layout/hit-test code paths that are neither named `const`s nor sourced from the sanctioned `Viewport`/`LayoutContext` type. This is the lint that makes the 1280px class un-reintroducible. It will need an allow-attribute escape hatch for genuinely intrinsic constants, each requiring a justifying comment.
- **`spdx_header_line_one`** — fails if line 1 is not the Apache-2.0 SPDX header.

### 6.3 CI shell gates

- **300-line ceiling** — a script (ripgrep/`wc`) failing on any tracked `.rs` file over 300 lines, with an explicit, reviewed exceptions file if any file legitimately must exceed it.
- **`forbid(unsafe_code)` presence** — verifies the attribute at each crate root except the named exceptions, which are themselves enumerated in a checked-in allow-list so new unsafe crates can't appear silently.
- **Dependency-direction check** — parses the workspace graph (e.g. via `cargo metadata`) and fails on any uphill edge per §5.1. This is what keeps the architecture from eroding after Spec 01 lands.

### 6.4 Relationship to other specs

The schema-validation and round-trip gates are defined by **Spec 02 (Testing)** and plug into the same CI pipeline. Benchmark regression tracking (**Spec 06**) is local-only and explicitly *not* a CI gate. This spec owns only the structural/convention gates above.

---

## 7. Key Decisions (ADR-style)

**D1 — Surface-and-triage, not pre-rank.** The audit reports counts and blast radius; the maintainer assigns priority. Rationale: severity depends on roadmap context the agent lacks. Tradeoff: an extra round-trip before fixes begin, accepted for correctness of prioritization.

**D2 — Enforcement lands before bulk fixes.** Gates are implemented early so every subsequent fix is verified by them. Tradeoff: the codebase will be red against its own gates briefly; mitigated by an allow-list/baseline so CI can distinguish "known debt being worked" from "new violation."

**D3 — Architecture is reachable, not ideal.** The proposed layering is derived from the existing graph; where the ideal is expensive, the ADR documents a pragmatic intermediate. Rationale: an unreachable target gets ignored.

**D4 — Single viewport source of truth.** All layout/render/hit-test dimensions flow from one `Viewport`/`LayoutContext` value rather than literals. This is both a fix and the precondition for Spec 03 (Responsive). Tradeoff: threading the context through call sites is invasive; the dylint ensures it's worth doing once.

---

## 8. Milestones & Acceptance Criteria

**M1 — Audit report.** Complete inventory across all §4.1 categories; dedicated 1280px-class trace (§4.2); raw dependency graph. *Accept:* every crate inspected; triage table populated except the Priority column; no code changed.

**M2 — Architecture ADR.** Target layering + invariants (§5), validated against the real graph, with per-boundary violation lists and migration paths. *Accept:* maintainer can read the ADR and locate every current violation it claims.

**M3 — Enforcement live.** §6 gates implemented and running in CI against a baseline/allow-list. *Accept:* introducing a fresh 1280-style literal, an over-ceiling file, a missing SPDX header, or an uphill dependency each fails CI in a test branch.

**M4 — Triaged fixes.** Work the ranked list top-down; one ADR per non-trivial fix; the `Viewport` source-of-truth replaces the 1280px class. *Accept:* the allow-list/baseline shrinks toward empty; no gate regressions; existing 509 tests green (plus any added).

---

## 9. Out of Scope

- New user-facing features.
- Performance/memory optimization beyond removing incidental waste (→ Spec 06).
- The responsive layout *behavior* itself (→ Spec 03); this spec only removes the hardcodes blocking it and establishes the `Viewport` type.
- Schema-validation and round-trip test gates (→ Spec 02); this spec only reserves their slot in CI.
- Presentation/Spreadsheet-specific audits beyond shared/duplicated code; each member app gets its own audit pass later, reusing the enforcement primitives this spec creates.
