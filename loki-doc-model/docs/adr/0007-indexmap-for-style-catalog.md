# ADR-0007: IndexMap for the Style Catalog

**Status:** Accepted  
**Date:** 2026-04-06  
**Deciders:** AppThere Engineering

---

## Context

`StyleCatalog` stores named styles in maps keyed by `StyleId`. The
choice of map type affects:

1. **Lookup performance** — O(1) vs. O(log n) vs. O(n).
2. **Iteration order** — relevant for serialization reproducibility.
3. **Dependency footprint** — adding a crate vs. using `std`.

Candidates:

| Type | Order | Lookup | Dependency |
|---|---|---|---|
| `std::collections::HashMap` | Non-deterministic | O(1) avg | None |
| `std::collections::BTreeMap` | Sorted by key | O(log n) | None |
| `indexmap::IndexMap` | Insertion order | O(1) avg | `indexmap` crate |

---

## Decision

Use **`indexmap::IndexMap`** for all four maps in `StyleCatalog`
(`paragraph_styles`, `character_styles`, `table_styles`,
`list_styles`).

---

## Rationale

**Insertion-order preservation is required for reproducible output.**

Both ODF and OOXML have conventional orderings for style entries in
their respective style XML files:

- ODF's `<office:styles>` typically lists styles in declaration order.
  Importers using `loki-odf` insert styles in parse order; exporters
  should emit them in the same order to produce reproducible (diffable)
  output.
- OOXML's `word/styles.xml` similarly has a conventional order where
  built-in styles precede custom styles.

`HashMap`'s non-deterministic iteration order would produce different
byte-for-byte output on each run, breaking:
- Reproducible builds.
- Round-trip identity tests (load file → save file → compare bytes).
- Git diffs (every save produces a different ordering of style elements).

`BTreeMap` would sort by `StyleId` string value, which does not
correspond to the declaration order expected by either format.

`IndexMap` provides O(1) lookup (same as `HashMap`) with insertion-order
iteration, exactly matching the requirement.

---

## Consequences

- `indexmap` is a public dependency of `loki-doc-model`.
- The `serde` feature of `indexmap` is activated when the `serde`
  feature of `loki-doc-model` is activated, enabling JSON serialization
  of the full `StyleCatalog` with stable key ordering.
- Callers who insert styles in the wrong order will get output with
  styles in that order — the catalog does not re-sort. This is correct;
  order is the caller's responsibility.

---

## References

- `indexmap` crate: <https://docs.rs/indexmap>
- ISO/IEC TR 29166:2011, §7.2.3 — Style XML structure comparison
- ODF 1.3 Specification, §16 — Styles
- ECMA-376 Part 1, §17.7 — Styles
