# ADR-0002: ISO/IEC TR 29166 as the Property Set Authority

**Status:** Accepted  
**Date:** 2026-04-06  
**Deciders:** AppThere Engineering

---

## Context

`CharProps` and `ParaProps` must enumerate the formatting properties
that can be expressed in a format-neutral way. Choosing an incomplete
set loses information; choosing an overly ODF-centric or OOXML-centric
set biases the model toward one format.

Two obvious candidates for the authority on which properties to include:

1. **Derive from ODF alone.** The ODF 1.3 specification lists
   `style:text-properties` and `style:paragraph-properties` attributes.
   But many of these are ODF-specific or have no OOXML counterpart.

2. **Derive from OOXML alone.** `w:rPr` and `w:pPr` in ECMA-376.
   Same bias problem in the other direction.

3. **Use ISO/IEC TR 29166:2011** — *Information technology — Document
   description and processing languages — Guidelines for translation
   between ISO/IEC 26300 (ODF) and ISO/IEC 29500 (OOXML)*.

---

## Decision

Use **TR 29166 §6.2.1 and §6.2.2** as the canonical authority for the
property sets in `CharProps` and `ParaProps` respectively.

TR 29166 §6.2.1 defines the "Text formatting" feature table: every
text-level formatting property that exists in both ODF and OOXML is
listed, with the ODF attribute name and the OOXML element/attribute
name for each.

TR 29166 §6.2.2 defines the "Paragraph formatting" feature table in
the same way.

Every property in those tables is represented as a field in `CharProps`
or `ParaProps`. Properties that exist in only one format, or that TR
29166 classifies as "difficult" to translate (§8.3.3), go into the
`ExtensionBag` field of the respective props struct.

---

## Rationale

TR 29166 was produced by ISO/IEC JTC 1 / SC 34 — the same committee
that is responsible for maintaining both ODF (ISO/IEC 26300) and OOXML
(ISO/IEC 29500). Its feature tables represent the technical consensus of
people who understand both formats deeply.

Using it as the authority ensures:

- Every property modelled has a defined mapping in both ODF and OOXML.
- Properties that only one format supports are not silently included,
  forcing format-specific data into `ExtensionBag` where it belongs.
- The property set has an external, citable reference rather than being
  the result of ad-hoc decisions.

---

## Consequences

- Some OOXML-only properties (content controls, `rsid*` revision-save
  IDs, certain complex field switches) are not in `CharProps`/`ParaProps`
  and go into `ExtensionBag`. This is correct behaviour.
- Some ODF-only properties (certain `style:*` presentation hints, ODF
  1.3 extensions) are likewise in `ExtensionBag`.
- Every field in `CharProps` and `ParaProps` has a doc comment that
  cites the TR 29166 §6.2.1 or §6.2.2 entry it corresponds to, plus
  the ODF attribute name and OOXML element/attribute name.

---

## References

- ISO/IEC TR 29166:2011, §6.2.1 — Text formatting feature table
- ISO/IEC TR 29166:2011, §6.2.2 — Paragraph formatting feature table
- ISO/IEC TR 29166:2011, §8.3.3 — "Difficult" translation classification
