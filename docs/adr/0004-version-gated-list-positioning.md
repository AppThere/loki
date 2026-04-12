# ADR-0004: Gate list indentation calculation on the ODF version

**Status:** Accepted  
**Date:** 2024-10-01  
**Deciders:** AppThere engineering

---

## Context

ODF uses two incompatible models for specifying list-level indentation:

**ODF 1.1 legacy model** (attributes on `text:list-level-properties`):

```xml
<text:list-level-properties
    text:space-before="0.25cm"
    text:min-label-width="0.25cm"/>
```

- `text:space-before` — distance from the left margin to the list label.
- `text:min-label-width` — minimum width of the label area.
- Total indent from margin = `space-before + min-label-width`.
- Hanging indent = `min-label-width`.

**ODF 1.2+ label-alignment model** (introduced in ODF 1.2, §19.880):

```xml
<style:list-level-label-alignment
    text:label-followed-by="listtab"
    fo:margin-left="1.27cm"
    fo:text-indent="-0.635cm"/>
```

- `fo:margin-left` — left edge of the text block (= indent start).
- `fo:text-indent` — negative value means hanging; stored as positive.

Real ODF 1.2+ documents may still include both sets of attributes for
backward compatibility with ODF 1.1 readers.

## Decision

`mapper/lists.rs:map_indentation` applies the ODF 1.2+ model when:
1. `OdfVersion::supports_label_alignment()` returns `true` (version ≥ 1.2), **and**
2. `OdfListLevel::label_followed_by` is `Some` (the ODF 1.2+ attribute is present).

Otherwise it falls back to the ODF 1.1 legacy model.

## Rationale

Preferring the label-alignment model when available gives more accurate
indentation for modern documents. Falling back on the legacy attributes
ensures ODF 1.1 documents still produce correct results.

The double condition (version gate **and** attribute presence) handles the
common case of ODF 1.2+ documents that include legacy attributes for
compatibility: if `label_followed_by` is absent the ODF 1.2+ block was not
present and the legacy values are authoritative.

## Consequences

- `OdfVersion::supports_label_alignment()` is the single authoritative gate.
- `OdfListLevel` retains both the legacy and ODF 1.2+ fields so either can
  be used depending on the gate.
- Round-tripping an ODF 1.1 document back to ODF 1.1 will use the legacy
  fields, preserving the original indentation values.
