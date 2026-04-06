# ADR-0003: Option\<T\> for Style Property Inheritance

**Status:** Accepted  
**Date:** 2026-04-06  
**Deciders:** AppThere Engineering

---

## Context

Both ODF and OOXML use a style inheritance model where formatting
properties cascade from parent style to child style with explicit
overrides at each level. A paragraph may belong to style "Heading1"
which is based on "Normal"; the "Heading1" style only overrides font
size and bold — all other properties are inherited from "Normal".

The model needs to distinguish between:
- A property that is **explicitly set** in a style or direct formatting.
- A property that is **unset** — to be inherited from the parent.

Two approaches were considered:

1. **Mandatory concrete values.** Every field in `CharProps` and
   `ParaProps` has a concrete value (e.g. `bool`, `Points`). A
   separate bitflag or `HashSet<PropertyKey>` records which fields are
   "explicitly set." Inheritance resolution merges the bitflag.

2. **`Option<T>` fields.** Every field is `Option<T>`. `None` means
   "unset; inherit from parent." `Some(v)` means "explicitly set to
   `v`." Inheritance resolution merges by: child wins when `Some`,
   falls back to parent when `None`.

---

## Decision

Use **`Option<T>` for every field** in `CharProps` and `ParaProps`.

`None` means "inherit from style/default." `Some(v)` means explicit
override to value `v`. A `CharProps` or `ParaProps` with all fields
`None` is a valid "inherit everything" state.

Style resolution is implemented in `StyleCatalog::resolve_para` and
`StyleCatalog::resolve_char` by walking the parent chain and calling
`merged_with_parent`, which fills in `None` fields from the parent.

---

## Rationale

The `Option<T>` approach directly models the "unset/set" distinction
that both ODF and OOXML make:

- ODF: a `style:text-properties` element that omits `fo:font-size`
  means "inherit font size from the parent style."
- OOXML: a `w:rPr` that omits `w:sz` means the same.

The encoding is natural, type-safe, and avoids the secondary bitflag
complexity. The `merge_with_parent` helper is a simple field-by-field
`Option::or` operation, easy to audit and test.

---

## Alternatives Rejected

- **Separate override bitflag.** More complex (two parallel data
  structures), no semantic advantage. The `Option` already encodes the
  same information.
- **Mandatory defaults everywhere.** Loses the "inherit" semantic
  entirely. Style resolution would require every style to replicate all
  properties from the root, or use a "magic default" sentinel value
  that is indistinguishable from an explicit setting.

---

## Consequences

- Every field in `CharProps` and `ParaProps` is wrapped in `Option<T>`.
  Consumers who want resolved (concrete) values call
  `StyleCatalog::resolve_para` / `resolve_char` which return a
  `ResolvedParaProps` / `ResolvedCharProps` (type aliases for the same
  structs — resolution fills in `None` fields but does not change the
  type).
- Direct formatting overrides on paragraphs and runs use the same
  `Option<T>` pattern: `None` in `direct_props` means "no direct
  override for this property."
- The `Default` impl for both structs leaves all fields as `None`,
  which is correct — it means "inherit everything."

---

## References

- ISO/IEC TR 29166:2011, §6.2.1 — text formatting properties (all
  listed as potentially absent in either format)
- ISO/IEC TR 29166:2011, §6.2.2 — paragraph formatting properties
