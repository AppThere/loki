// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Built-in heading styles (`Heading1`–`Heading6`) serializer.

use quick_xml::Writer;

use crate::docx::write::xml::{write_empty, write_end, write_start, wval};

/// Built-in heading level definitions: (styleId, display name, font size half-pts, bold, outline level).
pub(super) const HEADINGS: &[(&str, &str, i32, bool, u8)] = &[
    ("Heading1", "heading 1", 48, true, 0),
    ("Heading2", "heading 2", 36, true, 1),
    ("Heading3", "heading 3", 28, true, 2),
    ("Heading4", "heading 4", 24, true, 3),
    ("Heading5", "heading 5", 24, false, 4),
    ("Heading6", "heading 6", 20, false, 5),
];

pub(super) fn write_heading_styles<W: std::io::Write>(w: &mut Writer<W>) {
    for (style_id, name, sz_half_pts, bold, outline_lvl) in HEADINGS {
        let _ = write_start(
            w,
            "w:style",
            &[("w:type", "paragraph"), ("w:styleId", style_id)],
        );
        let _ = write_empty(w, "w:name", &wval(name));
        let _ = write_empty(w, "w:basedOn", &wval("Normal"));

        let _ = write_start(w, "w:pPr", &[]);
        let outline_s = outline_lvl.to_string();
        let _ = write_empty(w, "w:outlineLvl", &wval(&outline_s));
        let _ = write_end(w, "w:pPr");

        let _ = write_start(w, "w:rPr", &[]);
        if *bold {
            let _ = write_empty(w, "w:b", &[]);
        }
        let sz_s = sz_half_pts.to_string();
        let _ = write_empty(w, "w:sz", &wval(&sz_s));
        let _ = write_empty(w, "w:szCs", &wval(&sz_s));
        let _ = write_end(w, "w:rPr");

        let _ = write_end(w, "w:style");
    }
}
