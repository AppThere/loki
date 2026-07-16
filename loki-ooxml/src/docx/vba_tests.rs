// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::io::Cursor;

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::io::macros::MacroPayloadKind;
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

use crate::docx::export::DocxMacroEnabledExport;
use crate::docx::import::{DocxImportOptions, DocxImporter};
use crate::vba::{REL_VBA_PROJECT, REL_WORD_VBA_DATA};

/// Fake but structurally valid VBA project bytes. Loki never parses these;
/// the test only checks they survive the round-trip verbatim.
const FAKE_VBA: &[u8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1FAKE-CFB-VBA-PROJECT-BYTES";
const FAKE_VBA_DATA: &[u8] = br#"<?xml version="1.0"?><wne:vbaData/>"#;

/// Builds an in-memory `.docm` package: minimal document body + a VBA project
/// (`vbaProject.bin`) and its `vbaData.xml`, wired with the standard MS rels.
fn build_docm() -> Vec<u8> {
    let mut pkg = Package::new();

    let doc_part = PartName::new("/word/document.xml").unwrap();
    let vba_part = PartName::new("/word/vbaProject.bin").unwrap();
    let vba_data_part = PartName::new("/word/vbaData.xml").unwrap();

    let body = br#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:body></w:document>"#;

    pkg.set_part(
        doc_part.clone(),
        PartData::new(
            body.to_vec(),
            "application/vnd.ms-word.document.macroEnabled.main+xml",
        ),
    );
    pkg.set_part(
        vba_part.clone(),
        PartData::new(FAKE_VBA.to_vec(), "application/vnd.ms-office.vbaProject"),
    );
    pkg.set_part(
        vba_data_part.clone(),
        PartData::new(
            FAKE_VBA_DATA.to_vec(),
            "application/vnd.ms-word.vbaData+xml",
        ),
    );

    pkg.content_type_map_mut().add_default(
        "rels",
        "application/vnd.openxmlformats-package.relationships+xml",
    );
    pkg.content_type_map_mut()
        .add_default("xml", "application/xml");
    pkg.content_type_map_mut().add_override(
        &doc_part,
        "application/vnd.ms-word.document.macroEnabled.main+xml",
    );
    pkg.content_type_map_mut()
        .add_override(&vba_part, "application/vnd.ms-office.vbaProject");
    pkg.content_type_map_mut()
        .add_override(&vba_data_part, "application/vnd.ms-word.vbaData+xml");

    pkg.relationships_mut()
        .add(Relationship {
            id: "rId1".into(),
            rel_type:
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
                    .into(),
            target: "word/document.xml".into(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();

    pkg.part_relationships_mut(&doc_part)
        .add(Relationship {
            id: "rId100".into(),
            rel_type: REL_VBA_PROJECT.into(),
            target: "vbaProject.bin".into(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();

    pkg.part_relationships_mut(&vba_part)
        .add(Relationship {
            id: "rId1".into(),
            rel_type: REL_WORD_VBA_DATA.into(),
            target: "vbaData.xml".into(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();

    let mut buf = Cursor::new(Vec::new());
    pkg.write(&mut buf).unwrap();
    buf.into_inner()
}

#[test]
fn import_collects_vba_payload() {
    let bytes = build_docm();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("import");
    let macros = result
        .document
        .source
        .as_ref()
        .and_then(|s| s.macros.as_ref())
        .expect("macro payload preserved");

    assert_eq!(macros.kind, MacroPayloadKind::OoxmlVba);
    let project = macros
        .parts
        .iter()
        .find(|p| p.name.ends_with("vbaProject.bin"))
        .expect("vbaProject.bin part");
    assert_eq!(project.bytes, FAKE_VBA);
    let data = macros
        .parts
        .iter()
        .find(|p| p.name.ends_with("vbaData.xml"))
        .expect("vbaData.xml part");
    assert_eq!(data.bytes, FAKE_VBA_DATA);
}

#[test]
fn macro_enabled_export_preserves_project_bytes() {
    let doc = import_doc(&build_docm());

    let mut out = Cursor::new(Vec::new());
    DocxMacroEnabledExport::export(&doc, &mut out, ()).expect("export");
    let reimported = import_doc(&out.into_inner());

    let macros = reimported
        .source
        .as_ref()
        .and_then(|s| s.macros.as_ref())
        .expect("macros survive the round-trip");
    let project = macros
        .parts
        .iter()
        .find(|p| p.name.ends_with("vbaProject.bin"))
        .expect("vbaProject.bin");
    assert_eq!(
        project.bytes, FAKE_VBA,
        "VBA project bytes must be verbatim"
    );

    // The payload hash (trust-store key) is stable across the round-trip.
    let original = import_doc(&build_docm());
    assert_eq!(
        original.source.unwrap().macros.unwrap().payload_hash(),
        macros.payload_hash(),
    );
}

#[test]
fn plain_export_strips_macros() {
    let doc = import_doc(&build_docm());

    let mut out = Cursor::new(Vec::new());
    crate::docx::export::DocxExport::export(&doc, &mut out, ()).expect("export");
    let reimported = import_doc(&out.into_inner());

    assert!(
        reimported
            .source
            .as_ref()
            .and_then(|s| s.macros.as_ref())
            .is_none(),
        "a plain .docx save must drop the VBA payload"
    );
}

fn import_doc(bytes: &[u8]) -> Document {
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes.to_vec()))
        .expect("import")
        .document
}
