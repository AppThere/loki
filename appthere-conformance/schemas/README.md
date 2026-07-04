<!--
SPDX-License-Identifier: Apache-2.0
-->

# Vendored schemas (Axis 1 — schema validation)

The conformance schema axis validates Loki's exported XML against the **official,
version-pinned** schemas, vendored here so validation is reproducible and offline
(Spec 02 **D6**). `appthere_conformance::schema::XmllintValidator` points
`xmllint` at these files.

> **Status:** the validator and its plumbing are implemented and tested (against
> small in-tree schemas in the unit tests). Vendoring the *full* official schema
> sets below — and validating real DOCX/ODT exports against them — is the
> immediate next step (Spec 02 M2 completion). They are not committed yet because
> each is a large third-party drop with its own license/version that the
> maintainer should pin deliberately.

## What to vendor, and where

| Path | Schema | Source / version to pin |
|------|--------|--------------------------|
| `ooxml/transitional/` | ISO/IEC 29500-1 **Transitional** XSDs (`wml.xsd`, `sml.xsd`, `dml-*.xsd`, `shared-*.xsd`, …) | ECMA-376 5th ed. Transitional schema bundle. Default validation target. |
| `ooxml/strict/` | ISO/IEC 29500 **Strict** XSDs | Same bundle, Strict. Opt-in stricter pass. |
| `opc/` | OPC: `opc-contentTypes.xsd`, `opc-relationships.xsd`, `opc-coreProperties.xsd` | ECMA-376 Part 2 (OPC). Exercises `loki-opc`. |
| `odf/` | OASIS ODF **RELAX NG** (`OpenDocument-v1.3-schema.rng`) + `OpenDocument-v1.3-manifest-schema.rng` | OASIS ODF 1.3 schema. |

Each subdirectory should also carry a `PROVENANCE.txt` recording the exact source
URL, version/edition, retrieval date, and upstream license, so the pin is
auditable.

## How the validator uses them

```rust
use appthere_conformance::schema::{SchemaKind, SchemaValidator, XmllintValidator};

let v = XmllintValidator::new()?;                       // fails loudly if xmllint absent
let report = v.validate_bytes(
    &exported_document_xml,
    std::path::Path::new("appthere-conformance/schemas/ooxml/transitional/wml.xsd"),
    SchemaKind::Xsd,
)?;
assert!(report.valid, "{:#?}", report.violations);      // first-class located violations
```

ODF parts use `SchemaKind::RelaxNg` against the `.rng` files. A schema *registry*
that maps each emitted part (`document.xml`, `styles.xml`, `content.xml`,
`manifest.xml`, `[Content_Types].xml`, …) to its schema file is the small wrapper
to add once the files are vendored.
