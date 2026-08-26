// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for ODS spreadsheet import/export round-trip.

use loki_odf::{OdsExport, OdsImport, OdsImportOptions};
use loki_sheet_model::{CellAlign, CellStyle, DocumentMeta, NumberFormat, Workbook, Worksheet};
use std::io::{Cursor, Read};

#[test]
fn test_ods_round_trip() {
    let mut workbook = Workbook::new();

    // Set some metadata
    workbook.meta = DocumentMeta {
        title: Some("Test Spreadsheet".to_string()),
        creator: Some("Loki Test Builder".to_string()),
    };

    // Configure the first sheet
    {
        let sheet = workbook.get_sheet_mut(0).unwrap();
        sheet.name = "Data".to_string();

        // 1. Text cell with custom formatting (Bold + Center)
        let cell_a1 = sheet.get_cell_mut(0, 0); // A1
        cell_a1.value = "Product".to_string();
        cell_a1.style = Some(CellStyle {
            bold: true,
            italic: false,
            underline: false,
            align: CellAlign::Center,
            num_format: NumberFormat::General,
        });

        // 2. Numeric cell with custom formatting (Italic + Right + Currency)
        let cell_b1 = sheet.get_cell_mut(0, 1); // B1
        cell_b1.value = "1500.5".to_string();
        cell_b1.style = Some(CellStyle {
            bold: false,
            italic: true,
            underline: true,
            align: CellAlign::Right,
            num_format: NumberFormat::Currency,
        });

        // 3. Boolean cell
        let cell_c1 = sheet.get_cell_mut(0, 2); // C1
        cell_c1.value = "true".to_string();

        // 4. Introduce a gap to test `number-columns-repeated`
        // We write to E1 (col 4), leaving D1 (col 3) empty.
        let cell_e1 = sheet.get_cell_mut(0, 4); // E1
        cell_e1.value = "GapTest".to_string();

        // 5. Formula cell (SUM)
        let cell_a2 = sheet.get_cell_mut(1, 0); // A2
        cell_a2.value = "10".to_string();
        let cell_b2 = sheet.get_cell_mut(1, 1); // B2
        cell_b2.value = "20".to_string();
        let cell_c2 = sheet.get_cell_mut(1, 2); // C2
        cell_c2.formula = Some("SUM(A2:B2)".to_string());
        cell_c2.value = "30".to_string(); // evaluated value

        // 6. Percentage style
        let cell_a3 = sheet.get_cell_mut(2, 0); // A3
        cell_a3.value = "0.75".to_string();
        cell_a3.style = Some(CellStyle {
            bold: false,
            italic: false,
            underline: false,
            align: CellAlign::Left,
            num_format: NumberFormat::Percent,
        });
    }

    // Add a second sheet
    workbook.sheets.push(Worksheet::new("Summary"));
    {
        let sheet2 = workbook.get_sheet_mut(1).unwrap();
        let cell_a1 = sheet2.get_cell_mut(0, 0);
        cell_a1.value = "Total Summary".to_string();
    }

    // Export to buffer
    let mut buffer = Vec::new();
    OdsExport::export(&workbook, Cursor::new(&mut buffer)).expect("ODS export failed");

    // Import from buffer
    let imported_workbook = OdsImport::import(Cursor::new(buffer), OdsImportOptions::default())
        .expect("ODS import failed");

    // Assert structures
    assert_eq!(
        imported_workbook.sheets.len(),
        workbook.sheets.len(),
        "Number of sheets matches"
    );

    // Check Sheet 1 (Data)
    let sheet1_exp = &workbook.sheets[0];
    let sheet1_imp = &imported_workbook.sheets[0];
    assert_eq!(sheet1_imp.name, sheet1_exp.name);

    // Verify A1
    let a1_imp = sheet1_imp.get_cell(0, 0).expect("A1 missing");
    assert_eq!(a1_imp.value, "Product");
    let style_a1 = a1_imp.style.as_ref().expect("A1 style missing");
    assert!(style_a1.bold);
    assert_eq!(style_a1.align, CellAlign::Center);

    // Verify B1
    let b1_imp = sheet1_imp.get_cell(0, 1).expect("B1 missing");
    assert_eq!(b1_imp.value, "1500.5");
    let style_b1 = b1_imp.style.as_ref().expect("B1 style missing");
    assert!(style_b1.italic);
    assert!(style_b1.underline);
    assert_eq!(style_b1.align, CellAlign::Right);
    assert_eq!(style_b1.num_format, NumberFormat::Currency);

    // Verify C1
    let c1_imp = sheet1_imp.get_cell(0, 2).expect("C1 missing");
    assert_eq!(c1_imp.value, "true");

    // Verify D1 is empty/None (the gap)
    assert!(sheet1_imp.get_cell(0, 3).is_none(), "D1 should be empty");

    // Verify E1 (after the gap)
    let e1_imp = sheet1_imp.get_cell(0, 4).expect("E1 missing");
    assert_eq!(e1_imp.value, "GapTest");

    // Verify Formula C2
    let c2_imp = sheet1_imp.get_cell(1, 2).expect("C2 missing");
    assert_eq!(c2_imp.formula.as_deref(), Some("=SUM(A2:B2)"));
    assert_eq!(c2_imp.value, "30");

    // Verify Percentage A3
    let a3_imp = sheet1_imp.get_cell(2, 0).expect("A3 missing");
    assert_eq!(a3_imp.value, "0.75");
    let style_a3 = a3_imp.style.as_ref().expect("A3 style missing");
    assert_eq!(style_a3.num_format, NumberFormat::Percent);

    // Check Sheet 2 (Summary)
    let sheet2_imp = &imported_workbook.sheets[1];
    assert_eq!(sheet2_imp.name, "Summary");
    let s2_a1 = sheet2_imp.get_cell(0, 0).expect("Summary!A1 missing");
    assert_eq!(s2_a1.value, "Total Summary");
}

#[test]
fn workbook_metadata_survives_an_ods_round_trip() {
    // The metadata the round-trip test above sets in its fixture but never
    // asserts after re-import. ODS carries it in `meta.xml` (ODF 1.3 §3.1)
    // exactly as ODT does; without this assertion the writer could omit the
    // part entirely — as it did until this test was written — and every
    // spreadsheet would silently lose its title and author on save.
    let mut workbook = Workbook::new();
    workbook.meta = DocumentMeta {
        title: Some("Quarterly Figures".to_string()),
        creator: Some("Ada Lovelace".to_string()),
    };

    let mut buffer = Vec::new();
    OdsExport::export(&workbook, Cursor::new(&mut buffer)).expect("ODS export failed");
    let imported = OdsImport::import(Cursor::new(buffer), OdsImportOptions::default())
        .expect("ODS import failed");

    assert_eq!(imported.meta.title.as_deref(), Some("Quarterly Figures"));
    assert_eq!(imported.meta.creator.as_deref(), Some("Ada Lovelace"));
}

#[test]
fn a_workbook_without_metadata_round_trips_as_empty_not_as_a_stale_value() {
    // The polarity of the test above: absent metadata must come back absent,
    // so that test passes because the values travelled rather than because
    // something invents them.
    let workbook = Workbook::new();
    assert_eq!(
        workbook.meta,
        DocumentMeta::default(),
        "fixture precondition"
    );

    let mut buffer = Vec::new();
    OdsExport::export(&workbook, Cursor::new(&mut buffer)).expect("ODS export failed");
    let imported = OdsImport::import(Cursor::new(buffer), OdsImportOptions::default())
        .expect("ODS import failed");

    assert_eq!(imported.meta.title, None);
    assert_eq!(imported.meta.creator, None);
}

#[test]
fn the_exported_ods_declares_its_meta_part_in_the_manifest() {
    // A round trip cannot see this: Loki's reader finds `meta.xml` by entry
    // name, so the part would survive an export→import cycle even if the
    // manifest never declared it — and every other ODF application, which
    // reads the manifest, would see a package with no metadata. This is the
    // interop half, and it is the half a symmetric test is blind to.
    let mut workbook = Workbook::new();
    workbook.meta = DocumentMeta {
        title: Some("Declared".to_string()),
        creator: None,
    };

    let mut buffer = Vec::new();
    OdsExport::export(&workbook, Cursor::new(&mut buffer)).expect("ODS export failed");

    let mut zip = zip::ZipArchive::new(Cursor::new(buffer)).expect("valid zip");

    let mut manifest = String::new();
    zip.by_name("META-INF/manifest.xml")
        .expect("manifest present")
        .read_to_string(&mut manifest)
        .expect("readable manifest");
    assert!(
        manifest.contains(r#"manifest:full-path="meta.xml""#),
        "meta.xml must be declared in the manifest; got:\n{manifest}"
    );

    let mut meta = String::new();
    zip.by_name("meta.xml")
        .expect("meta.xml present as a zip entry")
        .read_to_string(&mut meta)
        .expect("readable meta.xml");
    assert!(meta.contains("<dc:title>Declared</dc:title>"));
    // The namespace declarations a foreign reader needs to resolve those
    // prefixes at all.
    assert!(meta.contains("xmlns:dc=\"http://purl.org/dc/elements/1.1/\""));
    assert!(meta.contains("office:version="));
}
