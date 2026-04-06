# ADR-0005: Field Model with Known-Kind Enum and Snapshot Value

**Status:** Accepted  
**Date:** 2026-04-06  
**Deciders:** AppThere Engineering

---

## Context

Both ODF and OOXML support document fields — inline content that is
evaluated dynamically at render time (page numbers, dates,
cross-references, word counts, etc.).

TR 29166 §5.2.19 defines "document fields" as a first-class document
property category, and §4.2 classifies them as "dynamic content."

The two formats represent fields differently:

- **ODF:** Named element types (`text:page-number`, `text:date`,
  `text:bookmark-ref`, etc.). The field type is clear from the element
  name.
- **OOXML:** The "complex field" mechanism uses a `w:fldChar` instruction
  run followed by `w:instrText` with a field instruction string like
  `PAGE`, `DATE \@ "MMMM d, yyyy"`, `REF _Ref123456 \h`, etc. The
  field type must be parsed from the instruction string.

An additional challenge: many documents contain fields whose instruction
strings cannot be round-tripped to the other format (custom OOXML fields,
ODF user-defined fields, etc.). These must be preserved for lossless
round-trips within the same format.

Finally, a document may be displayed in an environment that cannot
evaluate fields (headless rendering, partial export). The last-rendered
value should be available for display without evaluation.

---

## Decision

`Field` has:

1. **`kind: FieldKind`** — An enum of known field types (`PageNumber`,
   `PageCount`, `Date`, `Time`, `Title`, `Author`, `Subject`,
   `FileName`, `WordCount`, `CrossReference`). Unknown fields are stored
   as `Raw { instruction: String }`.

2. **`current_value: Option<String>`** — The last-rendered snapshot of
   the field value, populated by format importers from the cached result
   in the source document.

3. **`extensions: ExtensionBag`** — For format-specific field metadata
   that cannot be mapped to the known fields (e.g. OOXML field switches
   other than the instruction string).

---

## Rationale

**Known-kind enum:** Format exporters need to serialize fields back to
format-specific syntax. If the field kind is known, the exporter can
produce the correct syntax. If it is `Raw`, the exporter reproduces the
original instruction string verbatim — correct only for the source
format.

**`Raw` variant:** Needed for lossless round-trips. A document with a
custom OOXML field (`MAILMERGE`, custom `DOCPROPERTY`, etc.) must survive
a load-save cycle in the same format without data loss.

**`current_value` snapshot:** Needed for:
- Display in environments that cannot evaluate fields.
- Export targets that represent fields as static text (e.g. plain-text
  export).
- Partial-update scenarios where only changed fields need re-evaluation.

---

## Consequences

- Format importers must parse field instruction strings (OOXML) or
  element names (ODF) to produce the appropriate `FieldKind` variant.
- Format exporters must serialize `FieldKind` back to format-specific
  field syntax. For `Raw`, they use the instruction string verbatim.
- Cross-format round-trips of `Raw` fields will lose the field
  (producing `current_value` as static text) — this is documented and
  intentional.

---

## References

- ISO/IEC TR 29166:2011, §5.2.19 — Document fields feature table
- ISO/IEC TR 29166:2011, §4.2 — Dynamic content property classification
- ECMA-376 Part 1, §17.16 — Fields and hyperlinks
- ODF 1.3 Specification, §7.7 — Text fields
