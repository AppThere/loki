// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::io::Cursor;

use loki_doc_model::io::macros::MacroPayloadKind;
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

use crate::vba::REL_VBA_PROJECT;
use crate::xlsx::export::XlsxExport;
use crate::xlsx::import::{XlsxImport, XlsxImportOptions};

const FAKE_VBA: &[u8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1FAKE-EXCEL-VBA-PROJECT";

/// Builds a minimal in-memory `.xlsm`: one worksheet + a `xl/vbaProject.bin`
/// wired from the workbook part via the standard MS `vbaProject` rel.
fn build_xlsm() -> Vec<u8> {
    let mut pkg = Package::new();

    let wb_part = PartName::new("/xl/workbook.xml").unwrap();
    let sheet_part = PartName::new("/xl/worksheets/sheet1.xml").unwrap();
    let vba_part = PartName::new("/xl/vbaProject.bin").unwrap();

    let wb_xml = br#"<?xml version="1.0"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    let sheet_xml = br#"<?xml version="1.0"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Hi</t></is></c></row></sheetData></worksheet>"#;

    pkg.set_part(
        wb_part.clone(),
        PartData::new(
            wb_xml.to_vec(),
            "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
        ),
    );
    pkg.set_part(
        sheet_part.clone(),
        PartData::new(
            sheet_xml.to_vec(),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
        ),
    );
    pkg.set_part(
        vba_part.clone(),
        PartData::new(FAKE_VBA.to_vec(), "application/vnd.ms-office.vbaProject"),
    );

    let ct = pkg.content_type_map_mut();
    ct.add_default(
        "rels",
        "application/vnd.openxmlformats-package.relationships+xml",
    );
    ct.add_default("xml", "application/xml");
    ct.add_override(
        &wb_part,
        "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
    );
    ct.add_override(
        &sheet_part,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
    );
    ct.add_override(&vba_part, "application/vnd.ms-office.vbaProject");

    pkg.relationships_mut()
        .add(Relationship {
            id: "rId1".into(),
            rel_type:
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
                    .into(),
            target: "xl/workbook.xml".into(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();

    let wb_rels = pkg.part_relationships_mut(&wb_part);
    wb_rels
        .add(Relationship {
            id: "rId1".into(),
            rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet".into(),
            target: "worksheets/sheet1.xml".into(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();
    wb_rels
        .add(Relationship {
            id: "rId2".into(),
            rel_type: REL_VBA_PROJECT.into(),
            target: "vbaProject.bin".into(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();

    let mut buf = Cursor::new(Vec::new());
    pkg.write(&mut buf).unwrap();
    buf.into_inner()
}

#[test]
fn import_collects_xlsm_vba_payload() {
    let result = XlsxImport::run(Cursor::new(build_xlsm()), XlsxImportOptions::default())
        .expect("import");
    let macros = result.macros.expect("macro payload preserved");
    assert_eq!(macros.kind, MacroPayloadKind::OoxmlVba);
    let project = macros
        .parts
        .iter()
        .find(|p| p.name.ends_with("vbaProject.bin"))
        .expect("vbaProject.bin part");
    assert_eq!(project.bytes, FAKE_VBA);
}

#[test]
fn macro_enabled_export_preserves_project_bytes() {
    let imported = XlsxImport::run(Cursor::new(build_xlsm()), XlsxImportOptions::default())
        .expect("import");
    let macros = imported.macros.clone().expect("macros present");

    let mut out = Cursor::new(Vec::new());
    XlsxExport::export_with_macros(&imported.workbook, &mut out, Some(&macros)).expect("export");

    let reimported = XlsxImport::run(Cursor::new(out.into_inner()), XlsxImportOptions::default())
        .expect("reimport");
    let re_macros = reimported.macros.expect("macros survive round-trip");
    let project = re_macros
        .parts
        .iter()
        .find(|p| p.name.ends_with("vbaProject.bin"))
        .expect("vbaProject.bin");
    assert_eq!(project.bytes, FAKE_VBA);
    assert_eq!(macros.payload_hash(), re_macros.payload_hash());
}

#[test]
fn plain_export_strips_macros() {
    let imported = XlsxImport::run(Cursor::new(build_xlsm()), XlsxImportOptions::default())
        .expect("import");

    let mut out = Cursor::new(Vec::new());
    XlsxExport::export(&imported.workbook, &mut out).expect("export");

    let reimported = XlsxImport::run(Cursor::new(out.into_inner()), XlsxImportOptions::default())
        .expect("reimport");
    assert!(
        reimported.macros.is_none(),
        "a plain .xlsx save must drop the VBA payload"
    );
}
