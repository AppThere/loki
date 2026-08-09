# ADR-0005: ODF fields are typed XML elements, not state-machine field characters

**Status:** Accepted  
**Date:** 2024-10-01  
**Deciders:** AppThere engineering

---

## Context

ODF and OOXML use fundamentally different representations for dynamic inline
content (page numbers, dates, cross-references, etc.):

**OOXML** uses a state-machine approach:
```xml
<w:r><w:fldChar w:fldCharType="begin"/></w:r>
<w:r><w:instrText> PAGE </w:instrText></w:r>
<w:r><w:fldChar w:fldCharType="separate"/></w:r>
<w:r><w:t>42</w:t></w:r>          <!-- cached display value -->
<w:r><w:fldChar w:fldCharType="end"/></w:r>
```

**ODF** uses a typed element per field kind, with the cached value as element
text content:
```xml
<text:page-number text:select-page="current">42</text:page-number>
<text:date office:date-value="2024-06-15">15 June 2024</text:date>
<text:bookmark-ref text:ref-name="sec1">Introduction</text:bookmark-ref>
```

## Decision

The ODF reader maps each `text:*` field element to a concrete `OdfField`
variant (e.g. `OdfField::PageNumber`, `OdfField::Date`). Unknown field
elements are intended to fall through to `OdfField::Unknown { local_name,
current_value }`. *(Not implemented: the reader currently returns `Other` for an
unmodelled field and the mapper drops it, so `OdfField::Unknown` is never
constructed — `TODO(odf-unknown-field)`.)*

The document mapper converts `OdfField` to a `loki_doc_model` `Field` with a
`FieldKind`:

| ODF element | `FieldKind` |
|---|---|
| `text:page-number` | `FieldKind::PageNumber` |
| `text:page-count` | `FieldKind::PageCount` |
| `text:date` | `FieldKind::Date { format }` |
| `text:time` | `FieldKind::Time { format }` |
| `text:title` | `FieldKind::Title` |
| `text:subject` | `FieldKind::Subject` |
| `text:author-name` | `FieldKind::Author` |
| `text:file-name` | `FieldKind::FileName` |
| `text:word-count` | `FieldKind::WordCount` |
| `text:bookmark-ref` | `FieldKind::CrossReference { target, format }` |
| `text:chapter`, unknown | `FieldKind::Raw { instruction }` |

`Field::current_value` is *meant to* store the element's text content (the
last-rendered snapshot), so headless/EPUB exporters can fall back to the cached
value. *(Implementation status: ODF import does **not** yet capture it — the
reader skips the field body and the mapper sets `current_value: None`, so an ODF
`text:date`/`text:page-number`'s cached display text is lost and Date/Time
`office:date-value` is dropped; a page-number renders `1`, others blank. DOCX
import does populate it. `TODO(odf-field-value)`.)*

## Rationale

ODF's element-per-kind model is simpler to parse than OOXML's state machine:
each `text:*` field element is self-contained and can be mapped immediately
on the `Start` or `Empty` event without buffering multiple events.

Storing unknown fields in `FieldKind::Raw` rather than discarding them is
*intended to* preserve lossless round-tripping within ODF — the instruction
string can be used to reconstruct the original element on export. *(Not yet
realised: unmodelled `text:*` field elements are currently dropped on import;
only the modelled set plus `text:chapter` reach `FieldKind::Raw`.)*

## Consequences

- The ODF reader does **not** need a field-state machine; each field element
  is mapped in a single match arm.
- The `loki_doc_model::content::field` module provides a format-neutral
  `Field` type that both ODF and OOXML map to, enabling cross-format
  field semantics for common kinds (page numbers, dates, etc.).
- Unknown or extension field elements are preserved in `FieldKind::Raw`
  rather than silently dropped.
