// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Generates the `xl/workbook.xml` part.

use loki_sheet_model::Workbook;

use super::util::escape_xml;

pub(super) fn generate_workbook_xml(workbook: &Workbook) -> String {
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
