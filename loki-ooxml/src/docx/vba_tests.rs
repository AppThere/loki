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

/// Builds an in-memory `.docm` package with the default fake VBA bytes.
fn build_docm() -> Vec<u8> {
    build_docm_with(FAKE_VBA)
}

/// Builds an in-memory `.docm` package: minimal document body + a VBA project
/// (`vbaProject.bin` = `vba`) and its `vbaData.xml`, wired with the standard MS
/// rels.
fn build_docm_with(vba: &[u8]) -> Vec<u8> {
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
        PartData::new(vba.to_vec(), "application/vnd.ms-office.vbaProject"),
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

/// Builds a *real* `vbaProject.bin` (valid CFB) with one module — a p-code
/// prefix ahead of its compressed source, plus a `_VBA_PROJECT` cache — so the
/// write-back has genuine p-code to strip.
#[allow(clippy::cast_possible_truncation)] // record/offset lengths are tiny
fn real_vba_project(module_src: &str) -> Vec<u8> {
    const PCODE: [u8; 8] = [0xEE; 8];
    fn rec(id: u16, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }
    let crlf = module_src.replace('\n', "\r\n");
    let source_container = loki_vba::compress(crlf.as_bytes());

    let mut dir = rec(0x0003, &1252u16.to_le_bytes()); // CODEPAGE
    dir.extend(rec(0x0019, b"Module1")); // MODULENAME
    dir.extend(rec(0x001A, b"Module1")); // MODULESTREAMNAME
    dir.extend(rec(0x0021, &[])); // MODULETYPE (procedural)
    dir.extend(rec(0x0031, &(PCODE.len() as u32).to_le_bytes())); // MODULEOFFSET
    dir.extend(rec(0x002B, &[])); // module terminator

    let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    comp.create_storage("/VBA").unwrap();
    write_stream(&mut comp, "/VBA/dir", &loki_vba::compress(&dir));
    let mut module_stream = PCODE.to_vec();
    module_stream.extend_from_slice(&source_container);
    write_stream(&mut comp, "/VBA/Module1", &module_stream);
    write_stream(&mut comp, "/VBA/_VBA_PROJECT", &[0xAA; 16]);
    comp.flush().unwrap();
    comp.into_inner().into_inner()
}

fn write_stream(comp: &mut cfb::CompoundFile<Cursor<Vec<u8>>>, path: &str, bytes: &[u8]) {
    use std::io::Write;
    comp.create_stream(path).unwrap().write_all(bytes).unwrap();
}

/// End-to-end macro-editor path (spec §3.4, Phase 7.7): import a `.docm` with a
/// real VBA project, edit a module through the source-only write-back, export
/// macro-enabled, and re-import — the reopened project must read back the edited
/// source (and still be a valid, readable project with its p-code stripped).
#[test]
fn edited_vba_source_survives_docm_round_trip() {
    use std::collections::BTreeMap;

    let docm = build_docm_with(&real_vba_project("Sub Original\n  MsgBox \"v1\"\nEnd Sub"));
    let mut doc = import_doc(&docm);

    // Edit Module1 exactly as the editor's save does: write-back → replace_part.
    let mut payload = doc.source.as_ref().unwrap().macros.clone().unwrap();
    let part_name = payload
        .parts
        .iter()
        .find(|p| p.name.ends_with("vbaProject.bin"))
        .unwrap()
        .name
        .clone();
    let original_bin = payload
        .parts
        .iter()
        .find(|p| p.name == part_name)
        .unwrap()
        .bytes
        .clone();
    let edits: BTreeMap<String, String> = [(
        "Module1".to_string(),
        "Sub Original\n  Beep\nEnd Sub".to_string(),
    )]
    .into_iter()
    .collect();
    let new_bin = loki_vba::write_source(&original_bin, &edits).expect("write-back");
    payload.replace_part(&part_name, new_bin);
    doc.source.as_mut().unwrap().macros = Some(payload);

    // Save macro-enabled and reopen.
    let mut out = Cursor::new(Vec::new());
    DocxMacroEnabledExport::export(&doc, &mut out, ()).expect("export");
    let reimported = import_doc(&out.into_inner());

    let macros = reimported.source.as_ref().unwrap().macros.as_ref().unwrap();
    let bin = &macros
        .parts
        .iter()
        .find(|p| p.name.ends_with("vbaProject.bin"))
        .unwrap()
        .bytes;
    let project = loki_vba::VbaProject::read(bin).expect("edited project still reads");
    let module = project
        .modules
        .iter()
        .find(|m| m.name == "Module1")
        .expect("Module1 present after reopen");
    assert_eq!(module.source, "Sub Original\n  Beep\nEnd Sub");
    assert!(
        !module.source.contains("MsgBox"),
        "original source must be gone"
    );

    // p-code was stripped: the module stream is a bare compressed container.
    let mut comp = cfb::CompoundFile::open(Cursor::new(bin.clone())).unwrap();
    let mut stream = comp.open_stream("/VBA/Module1").unwrap();
    let mut raw = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut raw).unwrap();
    assert_eq!(
        raw.first(),
        Some(&0x01),
        "source at offset 0, no p-code prefix"
    );
    assert!(!raw.contains(&0xEE), "fake p-code must be gone");
}
