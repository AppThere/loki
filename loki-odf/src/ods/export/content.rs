// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Generator for ODS content.xml.

use std::collections::HashMap;

use loki_sheet_model::{CellAlign, CellStyle, NumberFormat, Workbook, Worksheet, Cell};

use super::xml_utils::{escape_xml, to_ods_formula};

pub(super) fn generate_content(workbook: &Workbook) -> String {
    // Collect unique cell styles
    let mut unique_styles = HashMap::new();
    let mut style_counter = 0;
    for sheet in &workbook.sheets {
        for cell in sheet.cells.values() {
            if let Some(ref style) = cell.style {
                if !unique_styles.contains_key(style) {
                    style_counter += 1;
                    let name = format!("ce{}", style_counter);
                    unique_styles.insert(style.clone(), name);
                }
            }
        }
    }

    let mut content_xml = String::new();
    content_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" office:version="1.3">
  <office:automatic-styles>
    <number:percentage-style style:name="NPercent">
      <number:number number:decimal-places="1" number:min-integer-digits="1"/>
      <number:text>%</number:text>
    </number:percentage-style>
    <number:currency-style style:name="NCurrency">
      <number:currency-symbol number:language="en" number:country="US">$</number:currency-symbol>
      <number:number number:decimal-places="2" number:min-integer-digits="1"/>
    </number:currency-style>
"#);

    append_style_definitions(&mut content_xml, &unique_styles);

    content_xml.push_str(
        r#"  </office:automatic-styles>
  <office:body>
    <office:spreadsheet>
"#,
    );

    for sheet in &workbook.sheets {
        append_sheet(&mut content_xml, sheet, &unique_styles);
    }

    content_xml.push_str(
        r#"    </office:spreadsheet>
  </office:body>
</office:document-content>
"#,
    );

    content_xml
}

fn append_style_definitions(
    content_xml: &mut String,
    unique_styles: &HashMap<CellStyle, String>,
) {
    for (style, name) in unique_styles {
        content_xml.push_str(&format!(
            "    <style:style style:name=\"{}\" style:family=\"table-cell\"",
            name
        ));
        let data_style = match style.num_format {
            NumberFormat::Percent => Some("NPercent"),
            NumberFormat::Currency => Some("NCurrency"),
            NumberFormat::General => None,
        };
        if let Some(ds) = data_style {
            content_xml.push_str(&format!(" style:data-style-name=\"{}\"", ds));
        }
        content_xml.push_str(">\n");

        if style.bold || style.italic || style.underline {
            content_xml.push_str("      <style:text-properties");
            if style.bold {
                content_xml.push_str(" fo:font-weight=\"bold\"");
            }
            if style.italic {
                content_xml.push_str(" fo:font-style=\"italic\"");
            }
            if style.underline {
                content_xml.push_str(" style:text-underline-style=\"solid\" style:text-underline-width=\"auto\" style:text-underline-color=\"font-color\"");
            }
            content_xml.push_str("/>\n");
        }

        if style.align != CellAlign::Left {
            content_xml.push_str("      <style:paragraph-properties");
            match style.align {
                CellAlign::Center => content_xml.push_str(" fo:text-align=\"center\""),
                CellAlign::Right => content_xml.push_str(" fo:text-align=\"right\""),
                CellAlign::Left => {}
            }
            content_xml.push_str("/>\n");
        }

        content_xml.push_str("    </style:style>\n");
    }
}

fn append_sheet(
    content_xml: &mut String,
    sheet: &Worksheet,
    unique_styles: &HashMap<CellStyle, String>,
) {
    let escaped_name = escape_xml(&sheet.name);
    content_xml.push_str(&format!(
        "      <table:table table:name=\"{}\">\n",
        escaped_name
    ));

    if !sheet.cells.is_empty() {
        append_sheet_rows(content_xml, sheet, unique_styles);
    } else {
        content_xml.push_str("        <table:table-row>\n");
        content_xml.push_str("          <table:table-cell/>\n");
        content_xml.push_str("        </table:table-row>\n");
    }

    content_xml.push_str("      </table:table>\n");
}

fn append_sheet_rows(
    content_xml: &mut String,
    sheet: &Worksheet,
    unique_styles: &HashMap<CellStyle, String>,
) {
    let mut max_row = 0;
    let mut max_col = 0;
    for &(r, c) in sheet.cells.keys() {
        if r > max_row {
            max_row = r;
        }
        if c > max_col {
            max_col = c;
        }
    }

    let mut row_cells: HashMap<u32, HashMap<u32, &Cell>> = HashMap::new();
    for (&(r, c), cell) in &sheet.cells {
        row_cells.entry(r).or_default().insert(c, cell);
    }

    for r in 0..=max_row {
        content_xml.push_str("        <table:table-row>\n");

        let mut last_col = 0;
        if let Some(cells) = row_cells.get(&r) {
            let mut cols: Vec<&u32> = cells.keys().collect();
            cols.sort();

            for &c in cols {
                let gap = c - last_col;
                if gap > 0 {
                    content_xml.push_str(&format!(
                        "          <table:table-cell table:number-columns-repeated=\"{}\"/>\n",
                        gap
                    ));
                }

                // `cols` is derived from `cells.keys()`, so this lookup
                // should always succeed; skip on the impossible miss.
                let Some(cell) = cells.get(&c) else {
                    continue;
                };
                append_cell(content_xml, cell, unique_styles);

                last_col = c + 1;
            }
        }
        content_xml.push_str("        </table:table-row>\n");
    }
}

fn append_cell(
    content_xml: &mut String,
    cell: &Cell,
    unique_styles: &HashMap<CellStyle, String>,
) {
    let style_attr = if let Some(ref s) = cell.style {
        if let Some(style_name) = unique_styles.get(s) {
            format!(" table:style-name=\"{}\"", style_name)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let formula_attr = if let Some(ref f) = cell.formula {
        let formatted = to_ods_formula(f);
        format!(" table:formula=\"{}\"", escape_xml(&formatted))
    } else {
        String::new()
    };

    let val_str = &cell.value;
    let is_bool =
        val_str.eq_ignore_ascii_case("true") || val_str.eq_ignore_ascii_case("false");
    let is_num = val_str.parse::<f64>().is_ok();

    if is_bool {
        content_xml.push_str(&format!(
            "          <table:table-cell office:value-type=\"boolean\" office:boolean-value=\"{}\"{}{}>\n",
            val_str.to_lowercase(),
            style_attr,
            formula_attr
        ));
    } else if is_num {
        content_xml.push_str(&format!(
            "          <table:table-cell office:value-type=\"float\" office:value=\"{}\"{}{}>\n",
            val_str, style_attr, formula_attr
        ));
    } else {
        content_xml.push_str(&format!(
            "          <table:table-cell office:value-type=\"string\"{}{}>\n",
            style_attr, formula_attr
        ));
    }

    content_xml.push_str(&format!(
        "            <text:p>{}</text:p>\n",
        escape_xml(val_str)
    ));
    content_xml.push_str("          </table:table-cell>\n");
}
