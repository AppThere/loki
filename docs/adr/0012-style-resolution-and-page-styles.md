<!--
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0012: Style resolution provenance, and the page-style asymmetry

**Status:** Accepted
**Date:** 2026-06-30
**Deciders:** AppThere engineering
**Resolves:** [`spec-05-style-management-panel.md`](spec-05-style-management-panel.md) §5 (resolution model + page-style asymmetry); [`spec-05-style-audit.md`](spec-05-style-audit.md) findings SM-1, SM-2, SM-9, SM-10, SM-12

---

## Context

Spec 05 rebuilds the style-management panel around a **resolved-vs-overridden
inspector**: every property shows not just its effective value but *where that
value comes from*. The audit (SM-1) found resolution already exists for the
renderer — `StyleCatalog::resolve_para` / `resolve_char` walk the single-parent
chain and collapse properties (child wins) — but it returns **bare values with
no provenance**, `resolve_char` is misnamed (it walks paragraph styles, not the
character family; SM-2), and there is no cycle guard for the re-parenting the
panel will offer (SM-12).

Spec 05 §5 also calls out a structural asymmetry that must be decided "explicitly
in the resolution-model ADR": **ODF has named, catalogued page styles**
(`style:page-layout` + `style:master-page`); **OOXML has none** — page setup
lives in section properties (`w:sectPr`). Loki today mirrors OOXML: page geometry
is per-section in `layout::PageLayout`, with **no `page_styles` entry in the
catalog** (SM-9). The inspector pattern (a named, selectable, inheritable entry
per family) does not fit page styling as currently modelled.

This ADR records two decisions: the **provenance resolution model** (Spec 05 M1,
now implemented) and the **page-style representation** (decided here, realized
with the M6 page panel).

---

## Decision 1 — Provenance resolution over the single-parent tree

Resolution yields, per property, a **`(provenance, value)`** answer:

```
Local              — set on the queried style itself
Inherited(StyleId) — first ancestor in the style's own `parent` chain that sets it
Default            — not in that chain; supplied by the document default style
                     (OOXML docDefaults / ODF default-style fall-through)
FormatDefault      — unset everywhere; the engine supplies the value (value = None)
```

It is implemented in `loki-doc-model/src/style/resolve.rs` as:

- `Provenance` and `Resolved<T> { provenance, value: Option<T> }` (`value` is
  `None` only for `FormatDefault`).
- A **generic, getter-based** resolver rather than a giant per-property result
  struct: `resolve_para_chain(id, |s| …)` and `resolve_char_chain(id, |s| …)`.
  The getter reads one property's *local* value from a style, so a single method
  serves every property of a family, and the inspector resolves each row on
  demand (cheap, lazy — relevant to Spec 06's measurement).
- `resolve_para_chain` also serves a paragraph style's **run-default character
  properties** (`|s| s.char_props.…`), and `resolve_char_chain` is the
  standalone-`CharacterStyle` resolver the misnamed `resolve_char` never
  provided (SM-2). The collapsing `resolve_para`/`resolve_char` stay as the
  renderer's value-only fast path — this is additive.
- **Cycle/depth safety** everywhere: a visited-set plus the existing
  `MAX_STYLE_CHAIN_DEPTH = 32` cap. `para_reparent_cycles(child, new_parent)`
  (backed by `para_ancestors`) lets the panel **reject** a re-parent that would
  form a cycle, keeping each family a tree (Spec 05 §7 / D3).

Rationale for the `Default` vs `Inherited` distinction: when a style *explicitly*
bases on the document default, a property set there is `Inherited(<that name>)`
— more informative than a generic "Default". `Default` is reserved for the
`docDefaults` fall-through a style reaches **without** naming the default in its
chain. (Verified by tests in `resolve_tests.rs`.)

---

## Decision 2 — Page styling: ODF-native named page styles in the catalog

**We adopt the ODF model as the unified representation** (Spec 05 §5's lean):
page styling will be a **named, catalogued family** — a future
`StyleCatalog::page_styles` keyed by `StyleId`, each entry carrying page geometry
(size, margins, orientation, columns) and its header/footer master, derived from
today's `PageLayout`. On **export**:

- **ODT** writes them natively as `style:page-layout` + `style:master-page`.
- **DOCX** maps each page style to the **section properties** (`w:sectPr`) of the
  sections that use it — there is no named OOXML page style to write, so the
  mapping is page-style → section, the inverse of import.

Page styles are an **explicit exception to the inheritance tree (D3)**: neither
format gives page styles a `basedOn` parent, so the page family has **no
parent chain**. In the panel it therefore behaves like the **list family**
(SM-9): the inspector still shows each property with provenance — but only
`Local` (set on this page style) and `FormatDefault` apply, and the **tree view
degrades to a flat list** (Spec 05 §7's Compact degradation, here at every
width). The resolution layer needs no page-specific code: a non-inheriting
family is just a chain of length one.

### Why not the alternatives

- **Keep page setup per-section only, present sections as synthetic "page
  styles."** Rejected: the inspector assumes a stable, named, selectable entry;
  synthesizing one per section re-keys it on section identity (which shifts as
  the document is edited) and has no home for the ODF master-page concept on
  round-trip.
- **Invent a Loki-specific page-inheritance tree.** Rejected: neither format
  supports it, so it could not round-trip; D3 keeps the model matching the
  formats' *actual* structure rather than forcing a uniform tree.

---

## Consequences

- **M1 is unblocked and implemented** for resolution: the inspector (M2) reads
  `Resolved<T>` per property; re-parenting (M4) uses `para_reparent_cycles`.
- The page family is **decided but not yet built**: the `page_styles` catalog
  field, the import mapping from `PageLayout`/`sectPr`, and the DOCX export
  inverse land with the **M6 page panel** — not in M1. Until then the page panel
  is absent (no dead UI, per the Spec 04 capability-gate discipline).
- Page styles are documented as a **non-inheriting family**; the inspector and
  tree-view code must treat "no parent chain" as a first-class case (already true
  for lists).
- No change to the renderer's existing `resolve_para`/`resolve_char` value path;
  this ADR is additive. Spec 02's round-trip trust is assumed, not re-litigated.
