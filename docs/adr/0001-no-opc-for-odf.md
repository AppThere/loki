# ADR-0001: Use plain ZIP (not OPC) for ODF package reading

**Status:** Accepted  
**Date:** 2024-10-01  
**Deciders:** AppThere engineering

---

## Context

Both ODF and OOXML (Office Open XML) are ZIP-based container formats. The Loki
suite also ships `loki-opc`, which provides an Open Packaging Conventions (OPC)
reader used by `loki-ooxml`.

When `loki-odf` was started, two options existed for reading the ODF ZIP
container:

1. **Reuse `loki-opc`** — OPC is a superset of ZIP and already handles entry
   lookup, content-type maps, and relationship parts.
2. **Use the `zip` crate directly** — treat the ODF package as plain ZIP,
   parsing only the entries that ODF requires (`content.xml`, `styles.xml`,
   `meta.xml`, `META-INF/manifest.xml`).

## Decision

Use the `zip` crate directly (option 2).  `loki-odf` reads the package via
`OdfPackage::open`, which calls `zip::ZipArchive::new` and extracts entries
by name.

## Rationale

ODF and OPC are structurally different at the semantic level:

| Property | ODF | OPC |
|---|---|---|
| Manifest format | `META-INF/manifest.xml` (ODF §3.9) | `[Content_Types].xml` + `.rels` files |
| Relationships | None (parts referenced by well-known paths) | Explicit relationship parts per entry |
| Content types | Derived from `manifest:media-type` | `[Content_Types].xml` |
| Mimetype check | First entry must be `mimetype`, stored/uncompressed | Not applicable |

Mapping ODF onto OPC abstractions would require inventing a fake relationship
model and content-type map, adding complexity without benefit.  The plain-ZIP
approach is simpler, directly implements the ODF 1.3 §3.3/§3.4 rules, and
avoids a dependency on `loki-opc`.

## Consequences

- `loki-odf` depends on the `zip` crate, not `loki-opc`.
- The `mimetype` entry must be the **first** ZIP entry and **uncompressed**
  (stored); `OdfPackage::open` validates this and returns
  `OdfError::MalformedElement` on violation (ODF 1.3 §3.4).
- Images are collected from the `Pictures/` subtree by iterating ZIP entry
  names — no manifest parsing is required for the import path.
