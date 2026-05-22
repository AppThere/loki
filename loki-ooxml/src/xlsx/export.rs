// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! XLSX exporter.

use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::OoxmlError;
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};
use loki_sheet_model::{Cell, CellAlign, CellStyle, NumberFormat, Workbook, Worksheet};
use std::collections::HashMap;
use std::io::{Seek, Write};

/// Unit struct that implements XLSX spreadsheet export.
pub struct XlsxExport;

impl XlsxExport {
    /// Exports a workbook model and writes the XLSX ZIP bytes to the writer.
    #[allow(clippy::too_many_lines)]
    pub fn export(workbook: &Workbook, writer: impl Write + Seek) -> Result<(), OoxmlError> {
        let mut pkg = Package::new();

        // 1. Gather all unique non-default styles used in the workbook
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

        // 2. Gather shared strings
        let mut shared_strings = Vec::new();
        let mut shared_strings_map = HashMap::new();
        for sheet in &workbook.sheets {
            for cell in sheet.cells.values() {
                if !cell.value.is_empty() && cell.value.parse::<f64>().is_err() {
                    // Save as a shared string if it's not a number
                    if !shared_strings_map.contains_key(&cell.value) {
                        shared_strings_map.insert(cell.value.clone(), shared_strings.len());
                        shared_strings.push(cell.value.clone());
                    }
                }
            }
        }

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

// ── XML Generation Helpers ───────────────────────────────────────────────────

fn generate_workbook_xml(workbook: &Workbook) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    xml.push_str("<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n");
    xml.push_str("  <sheets>\n");
    for (i, sheet) in workbook.sheets.iter().enumerate() {
        let rel_id = format!("rId{}", i + 3);
        xml.push_str(&format!(
            "    <sheet name=\"{}\" sheetId=\"{}\" r:id=\"{}\"/>\n",
            escape_xml(&sheet.name),
            i + 1,
            rel_id
        ));
    }
    xml.push_str("  </sheets>\n");
    xml.push_str("</workbook>\n");
    xml
}

fn generate_styles_xml(unique_styles: &[CellStyle]) -> String {
    let mut fonts = vec![(false, false, false)]; // (bold, italic, underline)
    let mut style_to_font_idx = Vec::new();

    for s in unique_styles {
        let key = (s.bold, s.italic, s.underline);
        let idx = if let Some(pos) = fonts.iter().position(|&x| x == key) {
            pos
        } else {
            fonts.push(key);
            fonts.len() - 1
        };
        style_to_font_idx.push(idx);
    }

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    xml.push_str(
        "<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\n",
    );

    // Fonts
    xml.push_str(&format!("  <fonts count=\"{}\">\n", fonts.len()));
    for (bold, italic, underline) in fonts {
        xml.push_str("    <font>\n");
        xml.push_str("      <sz val=\"11\"/>\n");
        xml.push_str("      <color theme=\"1\"/>\n");
        xml.push_str("      <name val=\"Calibri\"/>\n");
        xml.push_str("      <family val=\"2\"/>\n");
        xml.push_str("      <scheme val=\"minor\"/>\n");
        if bold {
            xml.push_str("      <b/>\n");
        }
        if italic {
            xml.push_str("      <i/>\n");
        }
        if underline {
            xml.push_str("      <u/>\n");
        }
        xml.push_str("    </font>\n");
    }
    xml.push_str("  </fonts>\n");

    // Fills (minimum required)
    xml.push_str("  <fills count=\"2\">\n");
    xml.push_str("    <fill><patternFill patternType=\"none\"/></fill>\n");
    xml.push_str("    <fill><patternFill patternType=\"gray125\"/></fill>\n");
    xml.push_str("  </fills>\n");

    // Borders (minimum required)
    xml.push_str("  <borders count=\"1\">\n");
    xml.push_str("    <border><left/><right/><top/><bottom/><diagonal/></border>\n");
    xml.push_str("  </borders>\n");

    // cellStyleXfs (minimum required)
    xml.push_str("  <cellStyleXfs count=\"1\">\n");
    xml.push_str("    <xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/>\n");
    xml.push_str("  </cellStyleXfs>\n");

    // cellXfs
    let xf_count = unique_styles.len() + 1;
    xml.push_str(&format!("  <cellXfs count=\"{}\">\n", xf_count));
    // Index 0 default
    xml.push_str("    <xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\n");
    for (i, s) in unique_styles.iter().enumerate() {
        let font_idx = style_to_font_idx[i];
        let num_fmt_id = match s.num_format {
            NumberFormat::Percent => 9,
            NumberFormat::Currency => 44,
            NumberFormat::General => 0,
        };
        let align_str = match s.align {
            CellAlign::Center => Some("center"),
            CellAlign::Right => Some("right"),
            CellAlign::Left => Some("left"),
        };

        if let Some(align) = align_str {
            xml.push_str(&format!(
                "    <xf numFmtId=\"{}\" fontId=\"{}\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyAlignment=\"1\">\n",
                num_fmt_id, font_idx
            ));
            xml.push_str(&format!("      <alignment horizontal=\"{}\"/>\n", align));
            xml.push_str("    </xf>\n");
        } else {
            xml.push_str(&format!(
                "    <xf numFmtId=\"{}\" fontId=\"{}\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\n",
                num_fmt_id, font_idx
            ));
        }
    }
    xml.push_str("  </cellXfs>\n");

    // cellStyles
    xml.push_str("  <cellStyles count=\"1\">\n");
    xml.push_str("    <cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/>\n");
    xml.push_str("  </cellStyles>\n");

    xml.push_str("  <dxfs count=\"0\"/>\n");
    xml.push_str("  <tableStyles count=\"0\" defaultTableStyle=\"TableStyleMedium9\" defaultPivotStyle=\"PivotStyleLight16\"/>\n");
    xml.push_str("</styleSheet>\n");
    xml
}

fn generate_shared_strings_xml(shared_strings: &[String]) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    xml.push_str(&format!(
        "<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{}\" uniqueCount=\"{}\">\n",
        shared_strings.len(),
        shared_strings.len()
    ));
    for s in shared_strings {
        xml.push_str(&format!("  <si><t>{}</t></si>\n", escape_xml(s)));
    }
    xml.push_str("</sst>\n");
    xml
}

fn generate_sheet_xml(
    sheet: &Worksheet,
    shared_strings_map: &HashMap<String, usize>,
    unique_styles: &[CellStyle],
) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    xml.push_str("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n");
    xml.push_str("  <sheetData>\n");

    // Group and sort cells
    let mut rows: HashMap<u32, Vec<(u32, &Cell)>> = HashMap::new();
    for (&(r, c), cell) in &sheet.cells {
        rows.entry(r).or_default().push((c, cell));
    }

    let mut row_indices: Vec<u32> = rows.keys().cloned().collect();
    row_indices.sort_unstable();

    for r in row_indices {
        let mut cols = rows.remove(&r).unwrap();
        cols.sort_unstable_by_key(|&(c, _)| c);

        xml.push_str(&format!("    <row r=\"{}\">\n", r + 1));
        for (c, cell) in cols {
            let cell_ref = coord_to_cell_ref(r, c);

            let style_idx = if let Some(style) = &cell.style {
                unique_styles
                    .iter()
                    .position(|x| x == style)
                    .map(|pos| pos + 1)
                    .unwrap_or(0)
            } else {
                0
            };

            let mut style_attr = String::new();
            if style_idx > 0 {
                style_attr = format!(" s=\"{}\"", style_idx);
            }

            if cell.formula.is_none() && cell.value.is_empty() {
                xml.push_str(&format!("      <c r=\"{}\"{}/>\n", cell_ref, style_attr));
            } else {
                let (t_attr, v_val) = if cell.value.is_empty() {
                    (String::new(), String::new())
                } else if cell.value.parse::<f64>().is_ok() {
                    (String::new(), format!("<v>{}</v>", cell.value))
                } else {
                    if let Some(&idx) = shared_strings_map.get(&cell.value) {
                        (format!(" t=\"s\""), format!("<v>{}</v>", idx))
                    } else {
                        (
                            format!(" t=\"inlineStr\""),
                            format!("<is><t>{}</t></is>", escape_xml(&cell.value)),
                        )
                    }
                };

                let formula_xml = if let Some(formula) = &cell.formula {
                    let mut fmt_f = formula.clone();
                    if fmt_f.starts_with('=') {
                        fmt_f.remove(0);
                    }
                    format!("<f>{}</f>", escape_xml(&fmt_f))
                } else {
                    String::new()
                };

                xml.push_str(&format!(
                    "      <c r=\"{}\"{}{}>\n        {}{}\n      </c>\n",
                    cell_ref, style_attr, t_attr, formula_xml, v_val
                ));
            }
        }
        xml.push_str("    </row>\n");
    }

    xml.push_str("  </sheetData>\n");
    xml.push_str("</worksheet>\n");
    xml
}

// ── Escape Helper ────────────────────────────────────────────────────────────

fn escape_xml(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

// ── Coordinate Conversion Helpers ──────────────────────────────────────────

fn coord_to_cell_ref(row: u32, col: u32) -> String {
    let mut col_str = String::new();
    let mut temp = col + 1;
    while temp > 0 {
        let modulo = (temp - 1) % 26;
        col_str.insert(0, (b'A' + modulo as u8) as char);
        temp = (temp - 1) / 26;
    }
    format!("{}{}", col_str, row + 1)
}
