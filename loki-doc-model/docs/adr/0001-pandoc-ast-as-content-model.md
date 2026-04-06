# ADR-0001: Pandoc AST as the Content Model

**Status:** Accepted  
**Date:** 2026-04-06  
**Deciders:** AppThere Engineering

---

## Context

`loki-doc-model` needs a format-neutral `Block`/`Inline` hierarchy for
representing text document content. Both ODF (`text:p`, `text:h`,
`text:list`, etc.) and OOXML (`w:p`, `w:r`, `w:tbl`, etc.) can be
thought of as serialization targets for an abstract content graph; the
question is which abstract graph to use.

Several options were considered:

1. **ODF schema as the model.** Use ODF's element vocabulary as the
   canonical structure. Import from ODF is trivial; export to OOXML
   would treat OOXML as a second-class target.

2. **OOXML schema as the model.** Symmetric problem in the other
   direction.

3. **Invent a new hierarchy.** Design a novel `Block`/`Inline` tree
   from first principles. This requires the same design work that has
   already been done elsewhere, without the benefit of prior validation.

4. **Adopt Pandoc's `Text.Pandoc.Definition`.** Pandoc's
   `Block`/`Inline` hierarchy has been validated against 30+ formats
   over 18 years of active development and maintenance. It has a
   detailed formal specification and a large body of reference literature.

---

## Decision

Adopt **Pandoc's `Text.Pandoc.Definition`** `Block` and `Inline` types
as the content layer of `loki-doc-model`, with extensions for
office-document-specific content that pandoc intentionally omits.

The `Block` enum and `Inline` enum in this crate have one-to-one
correspondences with pandoc's variants, using the same names and the
same semantics. Citations, math, raw blocks/inlines, definition lists,
and other content types that exist in pandoc are present in this crate
even when ODF and OOXML do not support them natively.

Office-document extensions added beyond pandoc's vocabulary:

| New variant | Rationale |
|---|---|
| `Block::StyledPara` | Represents a paragraph with a named style reference. Pandoc's `Para` has no style concept. TR 29166 §7.2.3. |
| `Block::TableOfContents` | Generated TOC blocks. ODF `text:table-of-content`, OOXML TOC field. TR 29166 §6.2.7. |
| `Block::Index` | Generated indices. ODF `text:alphabetical-index` etc. TR 29166 §7.2.6. |
| `Block::NotesBlock` | Footnote/endnote collection regions. |
| `Inline::StyledRun` | A run with a character style reference and direct props. ODF `text:span`, OOXML `w:r`. TR 29166 §7.2.2. |
| `Inline::Field` | Dynamic content (page numbers, dates, cross-refs). TR 29166 §5.2.19. |
| `Inline::Comment` | Comment anchor points. |
| `Inline::Bookmark` | Bookmark start/end markers. |

---

## Rationale

- Pandoc's AST is the most carefully designed format-neutral document
  content model that exists in open source software.
- It has survived exposure to LaTeX, HTML, Microsoft Word (OOXML), ODT,
  Markdown, reStructuredText, AsciiDoc, MediaWiki, and many others.
- It is thoroughly documented in `Text.Pandoc.Definition` with Haddock
  comments for every variant.
- Adopting it gives implementors a large body of reference material and
  maps `loki-doc-model` content to a well-known vocabulary.
- The model is deliberately a **superset** of what any single format
  supports. Variants that neither ODF nor OOXML can represent
  round-trip via `RawBlock`/`RawInline` or are preserved in
  `ExtensionBag`. This is intentional.

---

## Consequences

- `Block` and `Inline` include variants that ODF/OOXML parsers may
  never produce (e.g. `Block::DefinitionList`, `Inline::Cite`). This is
  acceptable; the model is a superset, not an intersection.
- Importers map format-native structures to these variants; exporters do
  the reverse. The mapping is documented in each variant's doc comment.
- Content that cannot be mapped to a known variant is stored in
  `ExtensionBag` or `RawBlock`/`RawInline` for lossless format-internal
  round-trips.

---

## References

- Pandoc `Text.Pandoc.Definition`:
  <https://hackage.haskell.org/package/pandoc-types/docs/Text-Pandoc-Definition.html>
- ISO/IEC TR 29166:2011, §7.2 (XML structure analysis)
