// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODS exporter.

use loki_doc_model::io::macros::MacroPayload;
use loki_sheet_model::Workbook;
use std::io::{Seek, Write};
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use crate::constants::{ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_MIMETYPE, ENTRY_STYLES, MIME_ODS};
use crate::error::OdfError;

#[path = "export_content.rs"]
mod content;

use content::generate_content;

/// Options controlling ODS export behaviour.
#[derive(Debug, Clone, Default)]
pub struct OdsExportOptions {}

/// Unit struct that implements ODS spreadsheet export.
pub struct OdsExport;

impl OdsExport {
    /// Export a [`Workbook`] to an ODS writer.
    pub fn export(workbook: &Workbook, writer: impl Write + Seek) -> Result<(), OdfError> {
        Self::export_with_macros(workbook, writer, None)
    }

    /// Export a [`Workbook`], re-emitting a preserved StarBasic/script payload
    /// when `macros` is `Some` (spec §3.3). `None` drops any prior macros.
    pub fn export_with_macros(
        workbook: &Workbook,
        writer: impl Write + Seek,
        macros: Option<&MacroPayload>,
    ) -> Result<(), OdfError> {
        let scripts = crate::script_write::odf_script_payload(macros);
        let mut zip = ZipWriter::new(writer);

        // 1. mimetype (stored, uncompressed)
        let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        zip.start_file(ENTRY_MIMETYPE, stored)?;
        zip.write_all(MIME_ODS.as_bytes())?;

        let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

        // 2. META-INF/manifest.xml
        zip.start_file(ENTRY_MANIFEST, deflated)?;
        zip.write_all(generate_manifest(scripts).as_bytes())?;

        // 3. styles.xml
        zip.start_file(ENTRY_STYLES, deflated)?;
        zip.write_all(generate_styles().as_bytes())?;

        // 4. content.xml
        zip.start_file(ENTRY_CONTENT, deflated)?;
        zip.write_all(generate_content(workbook).as_bytes())?;

        // 5. preserved macro/script libraries (Basic/, Scripts/), verbatim.
        if let Some(payload) = scripts {
            crate::script_write::write_script_parts(&mut zip, payload)?;
        }

        zip.finish()?;

        Ok(())
    }
}

fn generate_manifest(scripts: Option<&MacroPayload>) -> String {
    let mut m = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">
  <manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>
  <manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
  <manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
"#,
    );
    if let Some(payload) = scripts {
        m.push_str(&crate::script_write::script_manifest_entries(payload));
    }
    m.push_str("</manifest:manifest>\n");
    m
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
