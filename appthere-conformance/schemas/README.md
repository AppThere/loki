<!--
SPDX-License-Identifier: Apache-2.0
-->

# Vendored schemas (Axis 1 — schema validation)

The conformance schema axis validates Loki's exported XML against the **official,
version-pinned** schemas, vendored here so validation is reproducible and offline
(Spec 02 **D6**). `appthere_conformance::schema::XmllintValidator` points
`xmllint` at these files.

> **Status (2026-07-05): vendored and live.** Real DOCX and ODT exports are
> validated against these schemas in `loki-ooxml/tests/schema_validation.rs`
> and `loki-odf/tests/schema_validation.rs` (Spec 02 M2 acceptance: valid
> exports pass, a deliberately malformed part fails, a missing `xmllint`
> fails loudly).

## Layout (as vendored)

| Path | Schema | Pinned version / source |
|------|--------|--------------------------|
| `ooxml/transitional/` | ISO/IEC 29500-4 **Transitional** XSDs (`wml.xsd`, `sml.xsd`, `pml.xsd`, `dml-main.xsd`, `shared-*.xsd`, `vml-*.xsd`, `xml.xsd`) | **ISO/IEC 29500-4:2016** electronic-insert bundle. Default validation target. |
| `ooxml/mce/` | Markup Compatibility & Extensibility (`mc.xsd`), imported by `wml.xsd` | Same bundle. |
| `opc/` | OPC: `opc-contentTypes.xsd`, `opc-relationships.xsd`, `opc-coreProperties.xsd`, `opc-digSig.xsd` | **ECMA-376 4th ed. Part 2**. Exercises `loki-opc` (`[Content_Types].xml`, `_rels/.rels`). |
| `odf/` | OASIS ODF **RELAX NG**: `OpenDocument-v1.3-schema.rng`, `-manifest-`, `-dsig-` | **OASIS ODF 1.3 OS**, via Maven `org.odftoolkit:odfvalidator:0.12.0`. |
| `mathml3/` | W3C MathML3 RELAX NG (referenced by ODF math content) | Same artifact. |

Each subdirectory carries a `PROVENANCE.txt` recording the exact source,
version/edition, retrieval date, upstream license, and a per-file sha256
manifest, so the pin is auditable.

**Not vendored (documented tails):**

- **Strict** ISO/IEC 29500 XSDs (`ooxml/strict/`) — the opt-in stricter pass.
  Vendor from the same ISO bundle when the Strict pass is scheduled.
- The Dublin Core XSDs (`dc.xsd`, `dcterms.xsd`, `dcmitype.xsd`) that
  `opc-coreProperties.xsd` imports by live URL — required before
  `docProps/core.xml` can be validated offline. No in-policy source was
  reachable when vendoring; see `TODO(conformance-schemas)` in
  `loki-ooxml/tests/schema_validation.rs`.

## How the validator uses them

```rust
use appthere_conformance::schema::{SchemaKind, SchemaValidator, XmllintValidator};

let v = XmllintValidator::new()?;                       // fails loudly if xmllint absent
let report = v.validate_bytes(
    &exported_document_xml,
    // canonicalize(): libxml2 treats one file reached via two path spellings
    // as two schema documents and then skips "duplicate" namespace imports.
    &schemas_dir.join("ooxml/transitional/wml.xsd").canonicalize()?,
    SchemaKind::Xsd,
)?;
assert!(report.valid, "{:#?}", report.violations);      // first-class located violations
```

ODF parts use `SchemaKind::RelaxNg` against the `.rng` files. The part→schema
mapping lives in the consumer tests (`schema_validation.rs` in `loki-ooxml` /
`loki-odf`); promote it into a shared registry here if a third consumer appears.
