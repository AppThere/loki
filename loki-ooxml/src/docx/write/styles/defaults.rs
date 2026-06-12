// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:docDefaults` and `Normal` style serializers.

use quick_xml::Writer;

use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::catalog::StyleId;

use crate::docx::write::xml::{write_empty, write_end, write_start, wval};

pub(super) fn write_doc_defaults<W: std::io::Write>(w: &mut Writer<W>) {
    let _ = write_start(w, "w:docDefaults", &[]);
    let _ = write_start(w, "w:rPrDefault", &[]);
    let _ = write_start(w, "w:rPr", &[]);
    // Default font 12pt (24 half-pts), Times New Roman
    let _ = write_empty(
        w,
        "w:rFonts",
        &[
            ("w:ascii", "Times New Roman"),
            ("w:hAnsi", "Times New Roman"),
            ("w:cs", "Times New Roman"),
        ],
    );
    let _ = write_empty(w, "w:sz", &wval("24"));
    let _ = write_empty(w, "w:szCs", &wval("24"));
    let _ = write_end(w, "w:rPr");
    let _ = write_end(w, "w:rPrDefault");
    let _ = write_start(w, "w:pPrDefault", &[]);
    let _ = write_start(w, "w:pPr", &[]);
    let _ = write_end(w, "w:pPr");
    let _ = write_end(w, "w:pPrDefault");
    let _ = write_end(w, "w:docDefaults");
}

pub(super) fn write_normal_style<W: std::io::Write>(w: &mut Writer<W>, catalog: &StyleCatalog) {
    let normal_id = StyleId::new("Normal");
    let is_default = catalog
        .paragraph_styles
        .get(&normal_id)
        .is_some_and(|s| s.is_default)
        || catalog.paragraph_styles.is_empty();

    let _ = write_start(
        w,
        "w:style",
        &[
            ("w:type", "paragraph"),
            ("w:default", if is_default { "1" } else { "0" }),
            ("w:styleId", "Normal"),
        ],
    );
    let _ = write_empty(w, "w:name", &wval("Normal"));
    let _ = write_end(w, "w:style");
}
