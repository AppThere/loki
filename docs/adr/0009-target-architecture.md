<!--
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0009: Target layering & crate-dependency invariants

**Status:** Proposed
**Date:** 2026-06-28
**Deciders:** AppThere engineering
**Companion to:** [`spec-01-codebase-audit-and-architecture.md`](spec-01-codebase-audit-and-architecture.md) (§5)
**Depends on:** [`spec-01-audit-report.md`](spec-01-audit-report.md) (M1 — raw dependency graph)

---

## Context

Spec 01 §5 asks for a target layering **derived from the crates that already
exist**, validated against the real `cargo metadata` graph, with the violations
the audit found against each boundary and a migration path to conformance
(**D3 — reachable, not ideal**). This ADR is the **M2** deliverable.

The good news from M1: the workspace is already a clean, near-acyclic layered
graph. The proposed architecture is therefore **descriptive of where the code
already is**, with exactly one boundary to repair (A-8) and two classification
refinements over the spec's first sketch. Nothing here is an idealised rewrite.

The spec's §5 sketch named `appthere-color` and a generic "loro_bridge" crate;
neither exists as a standalone member. `loro_bridge` is a **module inside
`loki-doc-model`** (`loki-doc-model/src/loro_bridge/`), and colour lives in
`loki-primitives` / `loki-graphics`. The layering below is corrected to the real
25-crate membership.

---

## Decision

Adopt the following **seven-layer** target, with dependencies flowing **strictly
downward** (a crate may depend only on its own layer or below). Layer 0 is the
leaf foundation.

```
 L6  app        loki-text · loki-spreadsheet · loki-presentation
                ───────────────────────────────────────────────────────────
 L5  ui /       appthere-ui · loki-app-shell
     app-shell  (design system, shared runtime services: SpellService, i18n)
                ───────────────────────────────────────────────────────────
 L4  render     loki-renderer · loki-vello · appthere-canvas · loki-render-cache · loki-render-cpu
                ───────────────────────────────────────────────────────────
 L3b export+    loki-pdf            (exporters that REUSE layout for positioning)
 L3  layout     loki-layout         (pagination · Parley text layout)
                ═══════════ UDOM waist — narrow, format-neutral boundary ════
 L2  io/serde   loki-odf · loki-ooxml · loki-opc · loki-epub
                (loro_bridge lives inside loki-doc-model, not here)
                ───────────────────────────────────────────────────────────
 L1  model      loki-doc-model · loki-sheet-model · loki-presentation-model
                ───────────────────────────────────────────────────────────
 L0  foundation loki-primitives · loki-fonts · loki-graphics ·
                loki-i18n · loki-spell
```

The ACID test harness (`loki-acid`) sits outside the layering and depends across
it by design; the dependency-direction gate
(`scripts/check-dependency-direction.py`) exempts it by an explicit allow-list,
not silently. (`loki-templates` is *not* exempt — it is a real runtime content
dependency of the app binaries, mapped at **L2**.)

---

## The invariants (§5.1) and how each holds today

### I1 — Acyclic, downhill-only dependencies

`model` never imports `render`/`layout`/`ui`; `render` never imports `ui`.

**Status: holds with one exception.** Validated against the M1 graph:

- `loki-doc-model → loki-primitives` only. `loki-sheet-model → loki-primitives`.
  `loki-presentation-model → loki-graphics`. **No model crate imports
  layout/render/ui.** ✅
- `loki-layout → loki-doc-model, loki-fonts, loki-primitives, loki-spell` — all
  L0/L1, downhill. ✅
- `loki-vello → appthere-canvas, loki-layout` — L4/L3, downhill. ✅
- **Violation (A-8) — ✅ resolved.** `loki-renderer → appthere-ui` was a single
  **uphill** edge (render L4 → ui L5), via `use appthere_ui::tokens;` in
  `document_view.rs` for two constants (`PAGE_GAP_PX`, `SPACE_6`). **Fixed** (M-1):
  the two values are now **injected** into `DocumentViewProps` (`page_gap_px`,
  `content_padding_bottom_px`) by the app — which legitimately depends on
  `appthere_ui` — and the `appthere-ui` dependency is dropped from
  `loki-renderer/Cargo.toml`. The graph is now **fully downhill**, enforced by
  `scripts/check-dependency-direction.py` (A-13).

No cycles exist.

### I2 — The UDOM waist

Serialization (ODF/OOXML) talks to the model through a narrow, format-neutral
document interface; format-specific types do not leak upward into layout/render,
and render types do not leak down into serialization.

**Status: holds.** This is the strongest result of the audit:

- `loki-odf` and `loki-ooxml` depend **only downward** on model + foundation
  (`loki-doc-model`, `loki-sheet-model`, `loki-presentation-model`,
  `loki-primitives`, `loki-graphics`, `loki-opc`). They import **no** layout,
  render, or ui crate. ✅
- Conversely, `loki-layout` and `loki-renderer` import `loki-doc-model` (the
  neutral model) but **not** `loki-odf`/`loki-ooxml`. Format types therefore
  cannot reach layout/render. ✅
- Only the L6 app crates import both the model *and* the format crates — the
  correct place to wire IO.

The waist is real and intact; the gate's job (§6.3) is to keep `loki-odf` /
`loki-ooxml` out of every dependents' set above L2.

### I3 — Foundation purity

`loki-primitives`, `loki-fonts`, `loki-graphics` (and `loki-i18n`, `loki-spell`)
depend on nothing internal except, in a fixed order, each other.

**Status: holds.** `loki-primitives`, `loki-fonts`, `loki-i18n`, `loki-spell`
are leaves (no internal deps). `loki-graphics → loki-primitives` is the only
intra-foundation edge and is acyclic. ✅ (Note: the spec's `appthere-color` does
not exist; colour responsibilities live in `loki-primitives`/`loki-graphics`.)

### I4 — `forbid(unsafe_code)` everywhere except documented Android crates

**Status: holds in spirit, needs structural hardening.** 22 of 25 crate roots
carry `#![forbid(unsafe_code)]`. The three that don't —
`loki-text`, `loki-presentation`, `loki-spreadsheet` — are exactly the documented
Android `NativeActivity` FFI crates, and each `unsafe` call carries a `// SAFETY:`
comment (audit §3.3). The hardening (migration M-4): replace the *absence* of the
attribute with `#![deny(unsafe_code)]` + a narrowly-scoped `#[allow(unsafe_code)]`
on the FFI entry item, and enumerate the three crates in a checked-in allow-list
so a fourth unsafe crate cannot appear silently.

---

## Refinements over the spec's §5 sketch (D3)

Two places where the real graph diverges from the spec's first diagram, resolved
pragmatically rather than forced:

1. **Exporters split across the layout boundary (L3b).** The spec put all
   serialization in one io/serde band below layout. But `loki-pdf` *reuses*
   `loki-layout` for positioning (`loki-pdf → loki-layout`), so it cannot sit
   below layout. Resolution: pure-serialization exporters that touch only the
   model (`loki-epub → loki-doc-model, loki-primitives`; `loki-odf`; `loki-ooxml`;
   `loki-opc`) stay in the **L2 waist**; layout-consuming exporters (`loki-pdf`,
   and any future paginating exporter) form a thin **L3b "export-above-layout"**
   tier. This is a classification fix, **not** a code change (audit A-9).

2. **`loro_bridge` is not a crate.** It is a module tree inside `loki-doc-model`
   (`src/loro_bridge/`). The CRDT bridge therefore lives in **L1 (model)**, which
   is consistent with the waist: the bridge is part of the neutral model, not a
   format. The diagram is corrected accordingly.

3. **`appthere-canvas` naming vs. layer.** The `appthere-*` prefix elsewhere
   denotes the L5 UI design system, but `appthere-canvas` is a **render-layer**
   crate (`loki-vello → appthere-canvas`; `appthere-canvas → loki-render-cache`).
   Pure naming smell (audit-adjacent); no dependency violation. Left as-is, noted
   so the dependency gate's layer-assignment table is explicit rather than
   inferred from the prefix.

---

## Per-boundary violation list & migration paths (§5.2)

| Boundary / invariant | Violations found (M1) | Migration |
|---|---|---|
| I1 render ⊁ ui | `loki-renderer → appthere-ui` (`document_view.rs`, design tokens) | ✅ **Done (M-1)** — tokens injected via `DocumentViewProps`; `appthere-ui` dep dropped from `loki-renderer`. Enforced by the dependency-direction gate. |
| I2 waist | none | hold the line via the §6.3 gate (forbid `loki-odf`/`loki-ooxml` above L2) |
| I3 foundation purity | none | gate: foundation crates may import only foundation |
| I4 unsafe | 3 crates omit the attribute (expected) | **M-4:** `deny(unsafe_code)` + scoped `allow` + checked-in allow-list |
| L3b classification | `loki-pdf → loki-layout` (not a violation) | **M-2:** encode the L3b tier in the gate's layer map |

`loro_bridge` placement (**M-3**) and the `appthere-canvas` naming note are
documentation-only.

**Migration order.** M-1 is the only behaviourally-adjacent move and is small
(one `use` relocated to a lower crate). M-2/M-3 are layer-map entries. M-4 is
mechanical. None require a rewrite — consistent with D3: the target is reached by
*one* token relocation plus gate configuration, so the architecture is genuinely
*reachable*, not aspirational.

---

## Consequences

**Positive**

- The dependency-direction CI gate (Spec 01 §6.3) has an unambiguous,
  machine-checkable layer map to enforce, anchored to real crate names.
- Spec 03 (Responsive) inherits a clean render↔layout boundary once M-1 lands and
  the `Viewport` source-of-truth (D4 / audit A-1) is threaded.
- The waist (I2) is already intact, so format work and render work can proceed
  independently without re-introducing leakage — the gate makes that durable.

**Negative / costs**

- M-1 touches `loki-renderer` and wherever the relocated tokens are consumed; the
  token crate adds one foundation member. Small, but not zero.
- The layer map must be maintained as crates are added (mitigated: the gate fails
  on any unmapped crate, forcing an explicit layer assignment at PR time).

**Neutral**

- `loki-acid` / `loki-templates` are declared test/support members and exempted
  from the gate by explicit allow-list, not by silence.

---

## Validation appendix — layer assignment table (gate input)

| Crate | Layer | Internal deps (must all be ≤ layer) |
|---|---|---|
| loki-primitives | L0 | — |
| loki-fonts | L0 | — |
| loki-i18n | L0 | — |
| loki-spell | L0 | — |
| loki-graphics | L0 | loki-primitives |
| loki-doc-model | L1 | loki-primitives |
| loki-sheet-model | L1 | loki-primitives |
| loki-presentation-model | L1 | loki-graphics |
| loki-opc | L2 | — |
| loki-odf | L2 | loki-doc-model, loki-primitives, loki-sheet-model |
| loki-ooxml | L2 | loki-doc-model, loki-graphics, loki-opc, loki-presentation-model, loki-primitives, loki-sheet-model |
| loki-epub | L2 | loki-doc-model, loki-primitives |
| loki-layout | L3 | loki-doc-model, loki-fonts, loki-primitives, loki-spell |
| loki-pdf | L3b | loki-doc-model, loki-layout, loki-primitives |
| loki-render-cache | L4 | — |
| appthere-canvas | L4 | loki-render-cache |
| loki-vello | L4 | appthere-canvas, loki-layout |
| loki-renderer | L4 | appthere-canvas, loki-doc-model, loki-layout, loki-vello (A-8 `appthere-ui` edge removed) |
| loki-render-cpu | L4 | loki-layout (deterministic CPU rasterizer; conformance candidate render path) |
| appthere-ui | L5 | loki-i18n |
| loki-app-shell | L5 | loki-i18n, loki-spell |
| loki-text | L6 | appthere-ui, loki-app-shell, loki-doc-model, loki-epub, loki-fonts, loki-i18n, loki-layout, loki-odf, loki-ooxml, loki-pdf, loki-renderer, loki-templates, loki-vello |
| loki-spreadsheet | L6 | appthere-ui, loki-app-shell, loki-doc-model, loki-fonts, loki-i18n, loki-layout, loki-odf, loki-ooxml, loki-renderer, loki-sheet-model, loki-vello |
| loki-presentation | L6 | appthere-ui, loki-app-shell, loki-doc-model, loki-fonts, loki-graphics, loki-i18n, loki-layout, loki-odf, loki-ooxml, loki-presentation-model, loki-renderer, loki-vello |
| loki-acid | test | (exempt) |
| loki-templates | L2 | loki-doc-model, loki-ooxml, loki-primitives |
| appthere-conformance | test | (exempt) |
| loki-bench | test | (exempt — benchmark harness) |
| loki-model | L7 | — |
| loki-crypto | L7 | — |
| loki-server-audit | L7 | — |
| loki-print | L7 | — |
| loki-server-store | L8 | loki-crypto, loki-model, loki-server-audit |
| loki-server-auth | L8 | loki-model |
| loki-convert | L8 | loki-doc-model, loki-epub, loki-odf, loki-ooxml, loki-pdf, loki-sheet-model |
| loki-server-collab | L9 | loki-model, loki-server-store |
| loki-server-api | L10 | loki-crypto, loki-model, loki-server-audit, loki-server-auth, loki-server-collab, loki-server-store |
| loki-server | L11 | loki-crypto, loki-model, loki-server-api, loki-server-auth, loki-server-collab, loki-server-store |
| loki-headless | L11 | loki-convert, loki-print |

**Server subsystem (L7–L11)** was added when `main`'s web-server / headless
effort (spec ADRs C012–C028) merged in: a separate backend stack that consumes
the document/format libraries (`loki-convert` reaches down to `loki-pdf` at L3b)
but is never consumed by the client app binaries, so it sits above L6. The
layers order those crates by their own dependency graph.

The former A-8 `loki-renderer → appthere-ui` edge has been removed (M-1), so
**every edge is now conformant** — `scripts/check-dependency-direction.py`
verifies all internal edges flow downhill (35 mapped crates + the exempt
`loki-acid` / `appthere-conformance` / `loki-bench` harnesses) and fails CI on
any future uphill edge.
