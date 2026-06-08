// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODS exporter.

use loki_sheet_model::{Cell, CellAlign, NumberFormat, Workbook};
use std::collections::HashMap;
use std::io::{Seek, Write};
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use crate::constants::{ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_MIMETYPE, ENTRY_STYLES, MIME_ODS};
use crate::error::OdfError;

/// Options controlling ODS export behaviour.
#[derive(Debug, Clone, Default)]
pub struct OdsExportOptions {}

/// Unit struct that implements ODS spreadsheet export.
pub struct OdsExport;

impl OdsExport {
    /// Export a [`Workbook`] to an ODS writer.
    pub fn export(workbook: &Workbook, writer: impl Write + Seek) -> Result<(), OdfError> {
        let mut zip = ZipWriter::new(writer);

        // 1. mimetype (stored, uncompressed)
        let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        zip.start_file(ENTRY_MIMETYPE, stored)?;
        zip.write_all(MIME_ODS.as_bytes())?;

        let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

        // 2. META-INF/manifest.xml
        zip.start_file(ENTRY_MANIFEST, deflated)?;
        zip.write_all(generate_manifest().as_bytes())?;

        // 3. styles.xml
        zip.start_file(ENTRY_STYLES, deflated)?;
        zip.write_all(generate_styles().as_bytes())?;

        // 4. content.xml
        zip.start_file(ENTRY_CONTENT, deflated)?;
        zip.write_all(generate_content(workbook).as_bytes())?;

        zip.finish()?;

        Ok(())
    }
}

fn generate_manifest() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">
  <manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
</manifest:manifest>
"#.to_string()
}

fn generate_styles() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" office:version="1.3">
  <office:styles>
    <style:default-style style:family="table-cell"/>
  </office:styles>
</office:document-styles>
"#.to_string()
}

fn generate_content(workbook: &Workbook) -> String {
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

    for (style, name) in &unique_styles {
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

    content_xml.push_str(
        r#"  </office:automatic-styles>
  <office:body>
    <office:spreadsheet>
"#,
    );

    for sheet in &workbook.sheets {
        let escaped_name = escape_xml(&sheet.name);
        content_xml.push_str(&format!(
            "      <table:table table:name=\"{}\">\n",
            escaped_name
        ));

        if !sheet.cells.is_empty() {
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
                        let Some(cell) = cells.get(&c) else { continue; };
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
                        let is_bool = val_str.eq_ignore_ascii_case("true")
                            || val_str.eq_ignore_ascii_case("false");
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
                                val_str,
                                style_attr,
                                formula_attr
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

                        last_col = c + 1;
                    }
                }
                content_xml.push_str("        </table:table-row>\n");
            }
        } else {
            content_xml.push_str("        <table:table-row>\n");
            content_xml.push_str("          <table:table-cell/>\n");
            content_xml.push_str("        </table:table-row>\n");
        }

        content_xml.push_str("      </table:table>\n");
    }

    content_xml.push_str(
        r#"    </office:spreadsheet>
  </office:body>
</office:document-content>
"#,
    );

    content_xml
}

fn escape_xml(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(c),
        }
    }
    result
}

fn to_ods_formula(formula: &str) -> String {
    let s = formula.trim();
    let s = s.strip_prefix('=').unwrap_or(s);

    let mut result = String::new();
    result.push_str("of:=");

    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let mut has_dollar1 = false;
        if i < chars.len() && chars[i] == '$' {
            has_dollar1 = true;
            i += 1;
        }

        let mut letters_len = 0;
        while i < chars.len() && chars[i].is_ascii_alphabetic() {
            i += 1;
            letters_len += 1;
        }

        let mut has_dollar2 = false;
        if letters_len > 0 && i < chars.len() && chars[i] == '$' {
            has_dollar2 = true;
            i += 1;
        }

        let mut digits_len = 0;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
            digits_len += 1;
        }

        if letters_len > 0 && digits_len > 0 && letters_len <= 3 {
            let next_ok = i == chars.len() || !chars[i].is_ascii_alphanumeric();
            let prev_ok = start == 0 || !chars[start - 1].is_ascii_alphanumeric();

            if next_ok && prev_ok {
                let mut ref_str = String::new();
                if has_dollar1 {
                    ref_str.push('$');
                }
                for idx in (start + if has_dollar1 { 1 } else { 0 })
                    ..(i - digits_len - if has_dollar2 { 1 } else { 0 })
                {
                    ref_str.push(chars[idx]);
                }
                if has_dollar2 {
                    ref_str.push('$');
                }
                for idx in (i - digits_len)..i {
                    ref_str.push(chars[idx]);
                }
                result.push_str(&format!("[.{}]", ref_str));
                continue;
            }
        }

        i = start;
        result.push(chars[i]);
        i += 1;
    }
    result.replace("]:[", ":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_ods_formula() {
        assert_eq!(to_ods_formula("A1+B2"), "of:=[.A1]+[.B2]");
        assert_eq!(to_ods_formula("SUM(A1:B10)"), "of:=SUM([.A1:.B10])");
        assert_eq!(to_ods_formula("=SUM(A1:B10)"), "of:=SUM([.A1:.B10])");
        assert_eq!(
            to_ods_formula("A1:C5+SUM($D$2:D$8)"),
            "of:=[.A1:.C5]+SUM([.$D$2:.D$8])"
        );
    }
}
