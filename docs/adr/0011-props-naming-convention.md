<!--
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0011: `*Props` names two distinct concepts — and that's intentional

**Status:** Accepted
**Date:** 2026-06-28
**Deciders:** AppThere engineering
**Resolves:** [`spec-01-audit-report.md`](spec-01-audit-report.md) finding A-12

---

## Context

The Spec 01 audit (A-12) flagged the `*Props` suffix as overloaded: it names two
unrelated families of types.

1. **Document-model formatting bags** — `ParaProps`, `CharProps`, `RunProps`,
   `CellProps`, `TableProps`, `ShapeProps` (plus ODF variants `OdfParaProps`,
   `OdfTextProps`, `OdfCellProps`). These live under `loki_doc_model::style::props`
   (and the format crates) and mirror the OOXML vocabulary — `pPr` = paragraph
   properties, `rPr` = run properties. They are plain data structs describing how
   a paragraph/run/cell is formatted.
2. **Dioxus component props** — `AtTitleBarProps`, `DocumentViewProps`,
   `AtHomeTabProps`, … `#[derive(Props)]` structs in `appthere_ui` and the app
   crates that carry a component's inputs.

The audit rated this **low priority** and proposed an *optional* rename of the
model bags to `*Format` / `*Attrs`.

## Decision

**Keep `*Props` for both. Do not rename.** Document the convention instead:

- `*Props` under `loki_doc_model` / the format crates (`loki-ooxml`, `loki-odf`)
  = a **formatting property bag**, following the OOXML `pPr`/`rPr` domain
  vocabulary.
- `*Props` deriving `dioxus::Props` = a **UI component's props**.

The two never collide: they live in disjoint crates and module paths
(`loki_doc_model::style::props::*` vs. the UI components), are never imported into
the same scope, and one derives `Props` while the other does not. The suffix is
read in context, exactly like `Config`/`Options` elsewhere.

## Rationale & tradeoffs

- **Blast radius is disproportionate to the benefit.** A rename would touch
  **350+ references across 66+ files** — the core model, every format mapper
  (DOCX/ODT/XLSX import *and* export), the layout engine, and the Loro CRDT
  bridge. `ParaProps`/`CharProps` alone appear ~270 times.
- **Real regression surface.** The bags flow through `loro_bridge` (CRDT
  round-trip) and the OOXML/ODF mappers; a mechanical rename is *mostly* safe but
  the cost/risk is real for a purely cosmetic gain on a "low-priority" item.
- **The name is good domain vocabulary.** `ParaProps`/`CharProps` map directly to
  `pPr`/`rPr`, which every reader of the format code already knows. `*Format` /
  `*Attrs` would be *less* precise, not more.
- **No functional confusion exists.** The overload is nominal only; the type
  system and module paths keep them apart.

## Consequences

- This ADR is the convention of record; new model formatting bags use `*Props`,
  new UI component props use `*Props` (deriving `dioxus::Props`).
- If a future change ever places both in one scope (none is foreseen), the
  *model* bag is the one to disambiguate — rename it to `*Format` in a dedicated,
  separately-reviewed PR with the CRDT/mapper round-trip suite (Spec 02) green
  before and after. That PR is explicitly **not** bundled into the audit fix
  pass, because its risk profile is different from the mechanical hygiene fixes.

## Alternatives considered

- **Rename model bags to `*Format`/`*Attrs` now.** Rejected: 350+-reference,
  cross-crate, serialization-adjacent change for a cosmetic, low-priority concern.
  Available on explicit request as its own PR.
- **Rename the UI props instead.** Rejected: `#[derive(Props)]` is Dioxus's own
  convention; fighting it would surprise every Dioxus reader.
