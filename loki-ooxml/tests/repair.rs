// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end tests for the public DOCX repair API on a real OPC package.

use std::io::Cursor;

use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_ooxml::{DocxExport, DocxImport, analyze_docx, repair_docx};
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

const MT_DOC: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const REL_OFFICE_DOC: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";

/// Builds a minimal, valid `.docx` whose one paragraph has an **out-of-order**
/// `w:pPr` (`w:jc` before `w:spacing`) — the classic Word-rejecting corruption.
fn dirty_docx() -> Vec<u8> {
    let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:jc w:val="center"/><w:spacing w:after="120"/></w:pPr><w:r><w:t>Hi</w:t></w:r></w:p></w:body></w:document>"#;
    let mut pkg = Package::new();
    let doc = PartName::new("/word/document.xml").unwrap();
    pkg.set_part(doc.clone(), PartData::new(xml.to_vec(), MT_DOC));
    pkg.relationships_mut()
        .add(Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOC.into(),
            target: "word/document.xml".into(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();
    let ct = pkg.content_type_map_mut();
    ct.add_default(
        "rels",
        "application/vnd.openxmlformats-package.relationships+xml",
    );
    ct.add_default("xml", "application/xml");
    ct.add_override(&doc, MT_DOC);
    let mut buf = Cursor::new(Vec::new());
    pkg.write(&mut buf).unwrap();
    buf.into_inner()
}

#[test]
fn analyze_detects_out_of_order_ppr() {
    let report = analyze_docx(&dirty_docx()).expect("analyze");
    assert_eq!(report.findings.len(), 1, "one out-of-order container");
    assert_eq!(report.findings[0].container, "w:pPr");
    assert!(!report.repaired, "analyze must not claim to have repaired");
}

#[test]
fn repair_fixes_and_keeps_the_document_importable() {
    let bytes = dirty_docx();
    let (fixed, report) = repair_docx(&bytes).expect("repair");
    assert_eq!(report.findings.len(), 1);
    assert!(report.repaired);
    // The repaired package is now clean...
    assert!(analyze_docx(&fixed).expect("re-analyze").is_clean());
    // ...and still opens.
    DocxImport::import(Cursor::new(&fixed), Default::default()).expect("repaired imports");
}

#[test]
fn repairing_a_clean_document_is_a_no_op() {
    let (fixed, _) = repair_docx(&dirty_docx()).unwrap();
    let (again, report) = repair_docx(&fixed).unwrap();
    assert!(report.is_clean(), "already-clean doc yields no findings");
    assert_eq!(again, fixed, "clean input returned unchanged");
}

#[test]
fn loki_export_is_word_schema_clean() {
    // Regression: Loki's own DocxExport must emit schema-ordered parts (the
    // assembly canonicalises them), so a document Loki writes opens in Word.
    let doc = DocxImport::import(Cursor::new(dirty_docx()), Default::default()).unwrap();
    let mut out = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut out, ()).unwrap();
    let report = analyze_docx(&out.into_inner()).expect("analyze export");
    assert!(
        report.is_clean(),
        "Loki export must be Word-valid: {:?}",
        report.findings
    );
}
