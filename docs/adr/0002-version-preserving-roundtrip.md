# ADR-0002: Preserve the ODF version across the import/export round-trip

**Status:** Accepted  
**Date:** 2024-10-01  
**Deciders:** AppThere engineering

---

## Context

ODF 1.1, 1.2, and 1.3 differ in ways that affect serialisation:

- ODF 1.1 documents may omit `office:version`; it is mandatory in 1.2+.
- The label-alignment list positioning model was introduced in ODF 1.2
  (`text:list-level-position-and-space-mode="label-alignment"`).
- ODF 1.3 adds several new element types.

A round-trip workflow is common: open an existing ODT, make edits, save it
back. If the exporter silently upgrades the file to ODF 1.3, tools that only
support ODF 1.1 may fail to open the result.

## Decision

The detected `office:version` is stored in
`loki_doc_model::io::DocumentSource::version` and exposed via
`OdtImportResult::source_version`. Exporters read `document.source.version`
and write the same version string back.

Concretely:

- `office:version` absent → `OdfVersion::V1_1` (`source_version = V1_1`,
  `DocumentSource::version = Some("1.1")`).
- `office:version="1.2"` → `OdfVersion::V1_2`, `version = Some("1.2")`.
- Unrecognised value (e.g. `"99.0"`) with `strict_version = false` →
  `OdfVersion::V1_3`, `OdfWarning::UnrecognisedVersion` emitted.
- Unrecognised value with `strict_version = true` →
  `OdfError::UnsupportedVersion`.

## Rationale

Preserving the version is the least-surprise behaviour for editors:
a file opened at version 1.1 comes back as version 1.1. The alternative —
always exporting at the latest version — risks breaking existing workflows.

Storing the version on `DocumentSource` (not on the document body) keeps it
in the provenance layer and makes it easy for exporters to access without
coupling themselves to ODF-specific types.

## Consequences

- `OdtImporter::run` must set `document.source` after `map_document` returns,
  so the correct `source_version` (from the package open step) takes
  precedence over any version the body mapper might infer.
- Version-gated behaviour (e.g. list positioning, see ADR-0004) uses
  `OdfDocument::version` (set from the same detected value) rather than the
  `DocumentSource` field.
