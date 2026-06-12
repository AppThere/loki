// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Generates the `xl/styles.xml` part.

use loki_sheet_model::{CellAlign, CellStyle, NumberFormat};

pub(super) fn generate_styles_xml(unique_styles: &[CellStyle]) -> String {
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
