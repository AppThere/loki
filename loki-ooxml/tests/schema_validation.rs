// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 02 schema axis (M2) — real DOCX exports validate against the vendored
//! ECMA-376 Transitional XSDs.
//!
//! The main WordprocessingML parts the DOCX writer emits are checked against
//! the official schema via `appthere_conformance`'s `XmllintValidator`
//! (libxml2). A missing `xmllint` fails loudly rather than skipping (Spec 02
//! §5); CI installs `libxml2-utils`. The schemas are vendored +
//! version-pinned under `appthere-conformance/schemas/` (D6).

use std::io::{Cursor, Read};
use std::path::PathBuf;

use appthere_conformance::{SchemaKind, SchemaValidator, XmllintValidator};
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::CharProps;
use loki_ooxml::docx::export::DocxExport;

fn schema_wml() -> PathBuf {
    // Canonicalized so every xsd:import resolves to one canonical location —
    // libxml2 treats the same file reached via two path spellings as two
    // schema documents and then skips "duplicate" namespace imports.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../appthere-conformance/schemas/ooxml/transitional/wml.xsd")
        .canonicalize()
        .expect("vendored wml.xsd must exist")
}

/// A document exercising the writer's main WordprocessingML surfaces.
fn sample_document() -> Document {
    let bold = CharProps {
        bold: Some(true),
        ..Default::default()
    };
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
    ];
    d.sections = vec![s];
    d
}

fn export_docx(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("DOCX export should succeed");
    buf.into_inner()
}

fn part(docx: &[u8], name: &str) -> Vec<u8> {
    let mut zip = zip::ZipArchive::new(Cursor::new(docx)).expect("exported DOCX must be a ZIP");
    let mut file = zip
        .by_name(name)
        .unwrap_or_else(|_| panic!("exported DOCX must contain {name}"));
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("read part");
    bytes
}

fn assert_part_valid(part_name: &str) {
    let docx = export_docx(&sample_document());
    let xml = part(&docx, part_name);
    let validator = XmllintValidator::new().expect("xmllint must be installed (libxml2-utils)");
    let report = validator
        .validate_bytes(&xml, &schema_wml(), SchemaKind::Xsd)
        .expect("validation must run");
    assert!(
        report.valid,
        "{part_name} must be ECMA-376-Transitional-valid; violations: {:#?}",
        report.violations
    );
}

#[test]
fn docx_document_xml_is_schema_valid() {
    assert_part_valid("word/document.xml");
}

#[test]
fn docx_styles_xml_is_schema_valid() {
    assert_part_valid("word/styles.xml");
}

fn schema_opc(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../appthere-conformance/schemas/opc")
        .join(name)
        .canonicalize()
        .expect("vendored OPC schema must exist")
}

fn assert_opc_part_valid(part_name: &str, schema_name: &str) {
    let docx = export_docx(&sample_document());
    let xml = part(&docx, part_name);
    let validator = XmllintValidator::new().expect("xmllint must be installed");
    let report = validator
        .validate_bytes(&xml, &schema_opc(schema_name), SchemaKind::Xsd)
        .expect("validation must run");
    assert!(
        report.valid,
        "{part_name} must be OPC-schema-valid; violations: {:#?}",
        report.violations
    );
}

/// The OPC package layer (`loki-opc` output) must satisfy ECMA-376 Part 2.
#[test]
fn docx_content_types_are_opc_schema_valid() {
    assert_opc_part_valid("[Content_Types].xml", "opc-contentTypes.xsd");
}

#[test]
fn docx_package_relationships_are_opc_schema_valid() {
    assert_opc_part_valid("_rels/.rels", "opc-relationships.xsd");
}

// TODO(conformance-schemas): validate docProps/core.xml against
// opc-coreProperties.xsd once the Dublin Core XSDs it imports (dc.xsd,
// dcterms.xsd, dcmitype.xsd) are vendored — the schema references them by
// live dublincore.org URL, which offline validation (D6) cannot follow, and
// no in-policy source for them was reachable from this environment.

/// M2 acceptance: a deliberately malformed part must FAIL the gate.
#[test]
fn deliberately_malformed_document_fails_the_gate() {
    let docx = export_docx(&sample_document());
    let xml = String::from_utf8(part(&docx, "word/document.xml")).expect("document.xml is UTF-8");
    let broken = xml.replacen("<w:body>", "<w:body><w:bogus-element/>", 1);
    assert_ne!(xml, broken, "the malformation must have been injected");
    let validator = XmllintValidator::new().expect("xmllint must be installed");
    let report = validator
        .validate_bytes(broken.as_bytes(), &schema_wml(), SchemaKind::Xsd)
        .expect("validation must run");
    assert!(
        !report.valid,
        "an invented w:bogus-element must be rejected by the WML schema"
    );
}
