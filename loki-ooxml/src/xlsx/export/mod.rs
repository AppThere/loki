// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XLSX exporter.

mod util;
mod xml_sheet;
mod xml_styles;
mod xml_workbook;

use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::OoxmlError;
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};
use loki_sheet_model::{CellStyle, Workbook};
use std::collections::HashMap;
use std::io::{Seek, Write};

use xml_sheet::{generate_shared_strings_xml, generate_sheet_xml};
use xml_styles::generate_styles_xml;
use xml_workbook::generate_workbook_xml;

/// Unit struct that implements XLSX spreadsheet export.
pub struct XlsxExport;

impl XlsxExport {
    /// Exports a workbook model and writes the XLSX ZIP bytes to the writer.
    #[allow(clippy::too_many_lines)]
    pub fn export(workbook: &Workbook, writer: impl Write + Seek) -> Result<(), OoxmlError> {
        let mut pkg = Package::new();

        // 1. Gather all unique non-default styles used in the workbook
        let unique_styles = collect_unique_styles(workbook);

        // 2. Gather shared strings
        let (shared_strings, shared_strings_map) = collect_shared_strings(workbook);

        // 3. Define part names
        let workbook_part = PartName::new("/xl/workbook.xml").map_err(OoxmlError::Opc)?;
        let styles_part = PartName::new("/xl/styles.xml").map_err(OoxmlError::Opc)?;
        let ss_part = PartName::new("/xl/sharedStrings.xml").map_err(OoxmlError::Opc)?;

        // 4. Write parts
        // Workbook XML
        let workbook_xml = generate_workbook_xml(workbook);
        pkg.set_part(
            workbook_part.clone(),
            PartData::new(
                workbook_xml.into_bytes(),
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
            ),
        );

        // Styles XML
        let styles_xml = generate_styles_xml(&unique_styles);
        pkg.set_part(
            styles_part.clone(),
            PartData::new(
                styles_xml.into_bytes(),
                "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
            ),
        );

        // Shared Strings XML (if any)
        let has_shared_strings = !shared_strings.is_empty();
        if has_shared_strings {
            let ss_xml = generate_shared_strings_xml(&shared_strings);
            pkg.set_part(
                ss_part.clone(),
                PartData::new(
                    ss_xml.into_bytes(),
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml",
                ),
            );
        }

        // Worksheet XMLs
        for (i, sheet) in workbook.sheets.iter().enumerate() {
            let sheet_xml = generate_sheet_xml(sheet, &shared_strings_map, &unique_styles);
            let sheet_name = format!("/xl/worksheets/sheet{}.xml", i + 1);
            let sheet_part = PartName::new(sheet_name).map_err(OoxmlError::Opc)?;
            pkg.set_part(
                sheet_part,
                PartData::new(
                    sheet_xml.into_bytes(),
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
                ),
            );
        }

        // 5. Setup relationships
        // Root relation
        pkg.relationships_mut()
            .add(Relationship {
                id: "rId1".to_string(),
                rel_type: REL_OFFICE_DOCUMENT.to_string(),
                target: "xl/workbook.xml".to_string(),
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;

        // Workbook relation: styles.xml
        pkg.part_relationships_mut(&workbook_part)
            .add(Relationship {
                id: "rId1".to_string(),
                rel_type:
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
                        .to_string(),
                target: "styles.xml".to_string(),
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;

        // Workbook relation: sharedStrings.xml
        if has_shared_strings {
            pkg.part_relationships_mut(&workbook_part)
                .add(Relationship {
                    id: "rId2".to_string(),
                    rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings".to_string(),
                    target: "sharedStrings.xml".to_string(),
                    target_mode: TargetMode::Internal,
                })
                .map_err(OoxmlError::Opc)?;
        }

        // Workbook relation: worksheets
        for i in 0..workbook.sheets.len() {
            let rel_id = format!("rId{}", i + 3);
            let target = format!("worksheets/sheet{}.xml", i + 1);
            pkg.part_relationships_mut(&workbook_part)
                .add(Relationship {
                    id: rel_id,
                    rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet".to_string(),
                    target,
                    target_mode: TargetMode::Internal,
                })
                .map_err(OoxmlError::Opc)?;
        }

        // 6. Setup Content Types
        let ct = pkg.content_type_map_mut();
        ct.add_default(
            "rels",
            "application/vnd.openxmlformats-package.relationships+xml",
        );
        ct.add_default("xml", "application/xml");
        ct.add_override(
            &workbook_part,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        );
        ct.add_override(
            &styles_part,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
        );
        if has_shared_strings {
            ct.add_override(
                &ss_part,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml",
            );
        }
        for i in 0..workbook.sheets.len() {
            let sheet_name = format!("/xl/worksheets/sheet{}.xml", i + 1);
            let sheet_part = PartName::new(sheet_name).map_err(OoxmlError::Opc)?;
            ct.add_override(
                &sheet_part,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
            );
        }

        // 7. Write ZIP
        pkg.write(writer).map_err(OoxmlError::Opc)
    }
}

fn collect_unique_styles(workbook: &Workbook) -> Vec<CellStyle> {
    let mut unique_styles = Vec::new();
    for sheet in &workbook.sheets {
        for cell in sheet.cells.values() {
            if let Some(style) = &cell.style {
                if !unique_styles.contains(style) {
                    unique_styles.push(style.clone());
                }
            }
        }
    }
    unique_styles
}

fn collect_shared_strings(workbook: &Workbook) -> (Vec<String>, HashMap<String, usize>) {
    let mut shared_strings = Vec::new();
    let mut shared_strings_map = HashMap::new();
    for sheet in &workbook.sheets {
        for cell in sheet.cells.values() {
            if !cell.value.is_empty() && cell.value.parse::<f64>().is_err() {
                if !shared_strings_map.contains_key(&cell.value) {
                    shared_strings_map.insert(cell.value.clone(), shared_strings.len());
                    shared_strings.push(cell.value.clone());
                }
            }
        }
    }
    (shared_strings, shared_strings_map)
}
