// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Generates `xl/sharedStrings.xml` and `xl/worksheets/sheetN.xml` parts.

use std::collections::HashMap;

use loki_sheet_model::{Cell, CellStyle, Worksheet};

use super::util::{coord_to_cell_ref, escape_xml};

pub(super) fn generate_shared_strings_xml(shared_strings: &[String]) -> String {
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

pub(super) fn generate_sheet_xml(
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
        let Some(mut cols) = rows.remove(&r) else {
            continue;
        };
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
