# ADR-0006: Change Tracking Stored Opaquely

**Status:** Accepted  
**Date:** 2026-04-06  
**Deciders:** AppThere Engineering

---

## Context

Both ODF and OOXML support tracked changes — a record of insertions,
deletions, and formatting changes made by collaborators, with author
and timestamp metadata. TR 29166 §7.2.7 analyses the translation
between the two models and classifies it as "difficult."

TR 29166 §8.3.3 ("Difficult" translation category) explicitly names
change tracking as a category where automated translation is unlikely
to produce correct results.

The structural differences are fundamental:

**ODF:** Change tracking uses explicit `text:tracked-changes` /
`text:changed-region` markup that is separate from the content flow.
The changed content (both the original and the revision) may be
present simultaneously in different regions.

**OOXML:** Change tracking uses inline revision markup (`w:ins`,
`w:del`, `w:rPrChange`, `w:pPrChange`) that is interleaved with the
content runs. The markup is structurally incompatible with ODF's
separate-region model.

Attempting automated translation between these two representations
produces documents where:
- The visual result is identical but the revision history is distorted.
- Multi-author revision graphs may be flattened incorrectly.
- Format-round-trip identity (load ODF, save ODF) is broken if the
  tracked changes are reconstructed rather than preserved.

---

## Decision

`TrackedChange` is **structurally typed** for the metadata that is
common to both formats (author, date, kind: Insert/Delete/FormatChange/Move)
but stores the format-specific content in `raw: ExtensionBag`.

No translation between ODF and OOXML change tracking is attempted.
A document with ODF tracked changes saved as OOXML will **not** have
OOXML tracked changes — only the final (accepted) content is preserved.
This is documented.

`TrackedChangeKind` (`Insert`, `Delete`, `FormatChange`, `Move`) is
provided so that consumers can make high-level decisions (e.g. "accept
all changes before export") without parsing the raw format data.

---

## Rationale

The alternative — attempting translation — would require:

1. Parsing ODF `text:changed-region` regions, understanding the
   before/after content semantics, and reconstructing inline OOXML
   `w:ins`/`w:del` runs. This is semantically lossy because ODF and
   OOXML use different identity models for "what was changed."

2. The reverse direction has the same problem.

The cost of getting this wrong (corrupted revision history, incorrect
content in the accepted document) is higher than the cost of not
translating tracked changes at all. Users who need cross-format tracked
change fidelity should accept all changes before converting.

The opaque storage approach guarantees lossless round-trips within
the same format (ODF → ODF, OOXML → OOXML) which is the primary use
case for document editing.

---

## Consequences

- ODF documents with tracked changes can be loaded and saved as ODF
  with all tracked changes preserved (via `raw: ExtensionBag`).
- OOXML documents with tracked changes can be loaded and saved as OOXML
  with all tracked changes preserved.
- Converting a document with tracked changes from ODF to OOXML (or vice
  versa) loses tracked changes. The final accepted content is preserved.
  This is documented behaviour.
- A future version of this crate **may** introduce a translation layer
  if the semantics can be specified precisely. The `TrackedChangeKind`
  enum is forward-compatible with such an addition.

---

## References

- ISO/IEC TR 29166:2011, §7.2.7 — Change tracking XML structure
- ISO/IEC TR 29166:2011, §8.3.3 — "Difficult" translation category
- ODF 1.3 Specification, §5.5 — Change tracking
- ECMA-376 Part 1, §17.13 — Revision information
