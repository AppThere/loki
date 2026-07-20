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

const MT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
const MT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";

/// Builds a `.docx` whose `settings.xml` declares a `<w:endnotePr>` referencing
/// separator notes, but the package has **no** `endnotes.xml` to back them
/// (`footnotePr` *is* backed by `footnotes.xml`). Word reports an error in the
/// Endnotes stream; Loki opens it. Exercises the cross-part note-separator check.
fn dangling_endnote_pr_docx() -> Vec<u8> {
    let doc = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:body></w:document>"#;
    let settings = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnotePr><w:footnote w:id="-1"/><w:footnote w:id="0"/></w:footnotePr><w:endnotePr><w:endnote w:id="-1"/><w:endnote w:id="0"/></w:endnotePr></w:settings>"#;
    let footnotes = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote></w:footnotes>"#;

    let mut pkg = Package::new();
    let mut add = |path: &str, bytes: &[u8], mt: &str| {
        let pn = PartName::new(path).unwrap();
        pkg.set_part(pn.clone(), PartData::new(bytes.to_vec(), mt));
        pkg.content_type_map_mut().add_override(&pn, mt);
    };
    add("/word/document.xml", doc, MT_DOC);
    add("/word/settings.xml", settings, MT_SETTINGS);
    add("/word/footnotes.xml", footnotes, MT_FOOTNOTES);

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
    let mut buf = Cursor::new(Vec::new());
    pkg.write(&mut buf).unwrap();
    buf.into_inner()
}

#[test]
fn analyze_detects_dangling_endnote_pr_but_not_the_backed_footnote_pr() {
    let report = analyze_docx(&dangling_endnote_pr_docx()).expect("analyze");
    assert_eq!(report.findings.len(), 1, "only the endnotePr dangles");
    assert_eq!(report.findings[0].part, "word/settings.xml");
    assert_eq!(report.findings[0].container, "w:endnotePr");
    assert!(!report.repaired);
}

#[test]
fn repair_removes_dangling_endnote_pr_refs_and_keeps_footnote_pr() {
    let (fixed, report) = repair_docx(&dangling_endnote_pr_docx()).expect("repair");
    assert_eq!(report.findings.len(), 1);
    assert!(report.repaired);
    // Re-analysis is clean, and the footnotePr (backed by footnotes.xml) survived.
    assert!(analyze_docx(&fixed).expect("re-analyze").is_clean());
    let pkg = Package::open(Cursor::new(&fixed)).expect("reopen");
    let settings = pkg
        .part(&PartName::new("/word/settings.xml").unwrap())
        .expect("settings present");
    let settings = String::from_utf8_lossy(&settings.bytes);
    assert!(
        settings.contains("<w:footnote "),
        "footnotePr refs preserved: {settings}"
    );
    assert!(
        !settings.contains("<w:endnote "),
        "dangling endnote refs removed: {settings}"
    );
    DocxImport::import(Cursor::new(&fixed), Default::default()).expect("repaired imports");
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
