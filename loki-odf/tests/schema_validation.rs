// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 02 schema axis (M2) — real ODT exports validate against the vendored
//! OASIS ODF 1.3 RELAX NG schemas.
//!
//! Every part the ODT writer emits is checked against the official schema via
//! `appthere_conformance`'s `XmllintValidator` (libxml2). A missing `xmllint`
//! fails loudly rather than skipping (Spec 02 §5); CI installs
//! `libxml2-utils`. The schemas are vendored + version-pinned under
//! `appthere-conformance/schemas/` (D6) — see its `README.md` / PROVENANCE.

use std::io::{Cursor, Read};
use std::path::PathBuf;

use appthere_conformance::{SchemaKind, SchemaValidator, XmllintValidator};
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::CharProps;
use loki_odf::odt::export::{OdtExport, OdtExportOptions};

fn schema(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../appthere-conformance/schemas/odf")
        .join(name)
}

/// A document exercising the writer's main surfaces: heading, plain and
/// styled paragraphs, and a table.
fn sample_document() -> Document {
    let bold = CharProps {
        bold: Some(true),
        ..Default::default()
    };
    let table = Block::Table(Box::new(Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: vec![],
        head: TableHead {
            attr: NodeAttr::default(),
            rows: vec![],
        },
        bodies: vec![TableBody {
            attr: NodeAttr::default(),
            head_rows: vec![],
            body_rows: vec![Row::new(vec![
                Cell::simple(vec![Block::Para(vec![Inline::Str("a".into())])]),
                Cell::simple(vec![Block::Para(vec![Inline::Str("b".into())])]),
            ])],
        }],
        foot: TableFoot {
            attr: NodeAttr::default(),
            rows: vec![],
        },
    }));
    let mut d = Document::default();
    let mut s = Section::new();
    s.blocks = vec![
        Block::Heading(1, NodeAttr::default(), vec![Inline::Str("Title".into())]),
        Block::Para(vec![
            Inline::Str("Plain then ".into()),
            Inline::StyledRun(StyledRun {
                style_id: None,
                direct_props: Some(Box::new(bold)),
                content: vec![Inline::Str("bold".into())],
                attr: NodeAttr::default(),
            }),
        ]),
        table,
    ];
    d.sections = vec![s];
    d
}

fn export_odt(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    OdtExport::export(doc, &mut buf, OdtExportOptions::default())
        .expect("ODT export should succeed");
    buf.into_inner()
}

/// Extracts a named part from the exported ODT ZIP.
fn part(odt: &[u8], name: &str) -> Vec<u8> {
    let mut zip = zip::ZipArchive::new(Cursor::new(odt)).expect("exported ODT must be a ZIP");
    let mut file = zip
        .by_name(name)
        .unwrap_or_else(|_| panic!("exported ODT must contain {name}"));
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("read part");
    bytes
}

fn assert_part_valid(part_name: &str, schema_name: &str) {
    let odt = export_odt(&sample_document());
    let xml = part(&odt, part_name);
    let validator = XmllintValidator::new().expect("xmllint must be installed (libxml2-utils)");
    let report = validator
        .validate_bytes(&xml, &schema(schema_name), SchemaKind::RelaxNg)
        .expect("validation must run");
    assert!(
        report.valid,
        "{part_name} must be ODF-1.3-schema-valid; violations: {:#?}",
        report.violations
    );
}

#[test]
fn odt_content_xml_is_schema_valid() {
    assert_part_valid("content.xml", "OpenDocument-v1.3-schema.rng");
}

#[test]
fn odt_styles_xml_is_schema_valid() {
    assert_part_valid("styles.xml", "OpenDocument-v1.3-schema.rng");
}

#[test]
fn odt_meta_xml_is_schema_valid() {
    assert_part_valid("meta.xml", "OpenDocument-v1.3-schema.rng");
}

#[test]
fn odt_manifest_is_schema_valid() {
    assert_part_valid(
        "META-INF/manifest.xml",
        "OpenDocument-v1.3-manifest-schema.rng",
    );
}

/// M2 acceptance: a deliberately malformed part must FAIL the gate (guards
/// against a validator that silently passes everything).
#[test]
fn deliberately_malformed_content_fails_the_gate() {
    let odt = export_odt(&sample_document());
    let xml = String::from_utf8(part(&odt, "content.xml")).expect("content.xml is UTF-8");
    let broken = xml.replacen("<office:body>", "<office:body><office:bogus-element/>", 1);
    assert_ne!(xml, broken, "the malformation must have been injected");
    let validator = XmllintValidator::new().expect("xmllint must be installed");
    let report = validator
        .validate_bytes(
            broken.as_bytes(),
            &schema("OpenDocument-v1.3-schema.rng"),
            SchemaKind::RelaxNg,
        )
        .expect("validation must run");
    assert!(
        !report.valid,
        "an invented office:bogus-element must be rejected by the ODF schema"
    );
}
