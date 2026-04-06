# ADR-0004: Two-Level List Model (Style Reference + Level)

**Status:** Accepted  
**Date:** 2026-04-06  
**Deciders:** AppThere Engineering

---

## Context

ODF and OOXML represent lists with structurally different models:

**ODF:** `text:list-style` defines up to 10 indent levels (0–9). A
`text:list` element wraps `text:list-item` elements. The list structure
is explicit in the XML. Continuation lists and restarts are controlled
with `text:continue-numbering` / `text:start-value`.

**OOXML:** `w:abstractNum` defines the level properties (up to 9
levels, 0-indexed). `w:num` instances link to an `abstractNum` and can
override individual level properties. Paragraph list participation is
controlled via `w:numId` and `w:ilvl` in `w:pPr`. There is no explicit
list container element.

TR 29166 §7.2.5 analyses the translation between these models and
identifies it as "moderate" complexity.

---

## Decision

List definitions live in `StyleCatalog` as `ListStyle` entries identified
by `ListId`. Paragraphs reference a list via `ParaProps.list_id` and
`ParaProps.list_level` (0-indexed).

This is **explicitly modelled on OOXML's `abstractNum`/`numId` pattern**
because:

1. OOXML has no list container element — list membership is entirely a
   paragraph property. Mapping this to a structure with explicit list
   containers would require heuristic reconstruction and be lossy.

2. ODF's `text:list-style` is a style entity (named, in the style
   catalog) with per-level definitions, which maps naturally to
   `ListStyle`.

3. A paragraph's list membership is `(ListId, level)` — exactly
   `ParaProps.list_id` + `ParaProps.list_level`.

---

## Rationale

The two-level model (catalog entry + paragraph property reference) can
represent both ODF and OOXML lists without loss:

- **ODF import:** parse `text:list-style` → `ListStyle` in catalog;
  set `list_id` + `list_level` on paragraphs.
- **OOXML import:** parse `w:abstractNum` → `ListStyle` in catalog; map
  `w:numId` → `ListId`; map `w:ilvl` → `list_level`.
- **Rendering/export:** resolve `ListId` → `ListStyle` → `ListLevel` to
  get bullet/number properties.

---

## Consequences

- The content tree carries only a reference to a list style; actual list
  visual properties (bullet char, numbering scheme, indentation) require
  resolving `ListId → ListStyle → ListLevel`.
- There is no explicit "list container" block type. Consecutive
  paragraphs with the same `list_id` form a visual list.
- `Block::OrderedList` and `Block::BulletList` (from pandoc) remain for
  pandoc-AST compatibility but do not use `ListStyle`. They carry their
  formatting inline via `ListAttributes`.

---

## References

- ISO/IEC TR 29166:2011, §6.2.5 — List styles feature table
- ISO/IEC TR 29166:2011, §7.2.5 — List XML structure comparison
- ECMA-376 Part 1, §17.9 — Numbering
- ODF 1.3 Specification, §16.30 — `text:list-style`
