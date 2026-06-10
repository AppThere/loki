// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `word/styles.xml` serializer.
//!
//! Emits `docDefaults`, a `Normal` paragraph style, `Heading1`–`Heading6`,
//! and all named paragraph/character styles from the document's
//! [`StyleCatalog`].
//!
//! ECMA-376 §17.7 (Document Styles).

use quick_xml::Writer;

use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;

use crate::docx::write::xml::{
    NS_W, hex_color_val, pts_to_half_pts, pts_to_twips, write_decl, write_empty, write_end,
    write_start, wval,
};

/// Built-in heading level definitions: (styleId, display name, font size half-pts, bold, outline level).
const HEADINGS: &[(&str, &str, i32, bool, u8)] = &[
    ("Heading1", "heading 1", 48, true, 0),
    ("Heading2", "heading 2", 36, true, 1),
    ("Heading3", "heading 3", 28, true, 2),
    ("Heading4", "heading 4", 24, true, 3),
    ("Heading5", "heading 5", 24, false, 4),
    ("Heading6", "heading 6", 20, false, 5),
];

/// Serializes the document's style catalog to `word/styles.xml` bytes.
pub(super) fn write_styles_xml(catalog: &StyleCatalog) -> Vec<u8> {
    let mut out = Vec::new();
    let mut w = Writer::new(&mut out);
    let _ = write_decl(&mut w);

    let _ = write_start(
        &mut w,
        "w:styles",
        &[
            ("xmlns:w", NS_W),
            ("xmlns:r", crate::docx::write::xml::NS_R),
        ],
    );

    write_doc_defaults(&mut w);
    write_normal_style(&mut w, catalog);
    write_heading_styles(&mut w);
    write_catalog_styles(&mut w, catalog);

    let _ = write_end(&mut w, "w:styles");
    drop(w);
    out
}

fn write_doc_defaults<W: std::io::Write>(w: &mut Writer<W>) {
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

fn write_normal_style<W: std::io::Write>(w: &mut Writer<W>, catalog: &StyleCatalog) {
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

fn write_heading_styles<W: std::io::Write>(w: &mut Writer<W>) {
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

fn write_catalog_styles<W: std::io::Write>(w: &mut Writer<W>, catalog: &StyleCatalog) {
    for (id, style) in &catalog.paragraph_styles {
        let sid = id.as_str();
        // Skip Normal and Heading1-6 — already emitted above.
        if sid == "Normal" || HEADINGS.iter().any(|(h, _, _, _, _)| *h == sid) {
            continue;
        }
        let is_default_s = if style.is_default { "1" } else { "0" };
        let _ = write_start(
            w,
            "w:style",
            &[
                ("w:type", "paragraph"),
                ("w:default", is_default_s),
                ("w:styleId", sid),
            ],
        );
        let name = style.display_name.as_deref().unwrap_or(sid);
        let _ = write_empty(w, "w:name", &wval(name));
        if let Some(ref parent) = style.parent {
            let _ = write_empty(w, "w:basedOn", &wval(parent.as_str()));
        }
        write_para_props_elem(w, &style.para_props);
        write_char_props_elem(w, &style.char_props);
        let _ = write_end(w, "w:style");
    }

    for (id, style) in &catalog.character_styles {
        let sid = id.as_str();
        let _ = write_start(w, "w:style", &[("w:type", "character"), ("w:styleId", sid)]);
        let name = style.display_name.as_deref().unwrap_or(sid);
        let _ = write_empty(w, "w:name", &wval(name));
        if let Some(ref parent) = style.parent {
            let _ = write_empty(w, "w:basedOn", &wval(parent.as_str()));
        }
        write_char_props_elem(w, &style.char_props);
        let _ = write_end(w, "w:style");
    }
}

/// Writes a `<w:pPr>` element from [`ParaProps`] (empty if nothing set).
pub(super) fn write_para_props_elem<W: std::io::Write>(w: &mut Writer<W>, pp: &ParaProps) {
    let has_content = pp.alignment.is_some()
        || pp.indent_start.is_some()
        || pp.indent_end.is_some()
        || pp.indent_hanging.is_some()
        || pp.space_before.is_some()
        || pp.space_after.is_some();
    if !has_content {
        return;
    }
    let _ = write_start(w, "w:pPr", &[]);

    if let Some(align) = pp.alignment {
        use loki_doc_model::style::props::para_props::ParagraphAlignment;
        let jc = match align {
            ParagraphAlignment::Right => "right",
            ParagraphAlignment::Center => "center",
            ParagraphAlignment::Justify => "both",
            _ => "left",
        };
        let _ = write_empty(w, "w:jc", &wval(jc));
    }

    let has_ind =
        pp.indent_start.is_some() || pp.indent_end.is_some() || pp.indent_hanging.is_some();
    if has_ind {
        let left = pp.indent_start.map_or(0, |v| pts_to_twips(v.value()));
        let right = pp.indent_end.map_or(0, |v| pts_to_twips(v.value()));
        let hanging = pp.indent_hanging.map_or(0, |v| pts_to_twips(v.value()));
        let left_s = left.to_string();
        let right_s = right.to_string();
        let hanging_s = hanging.to_string();
        let mut ind_attrs: Vec<(&str, &str)> = Vec::new();
        if left != 0 {
            ind_attrs.push(("w:left", &left_s));
        }
        if right != 0 {
            ind_attrs.push(("w:right", &right_s));
        }
        if hanging != 0 {
            ind_attrs.push(("w:hanging", &hanging_s));
        }
        if !ind_attrs.is_empty() {
            let _ = write_empty(w, "w:ind", &ind_attrs);
        }
    }

    if pp.space_before.is_some() || pp.space_after.is_some() {
        use loki_doc_model::style::props::para_props::Spacing;
        let before = pp.space_before.map_or(0, |v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        });
        let after = pp.space_after.map_or(0, |v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        });
        let before_s = before.to_string();
        let after_s = after.to_string();
        let _ = write_empty(
            w,
            "w:spacing",
            &[("w:before", &before_s), ("w:after", &after_s)],
        );
    }

    let _ = write_end(w, "w:pPr");
}

/// Writes a `<w:rPr>` element from [`CharProps`] (empty if nothing set).
pub(super) fn write_char_props_elem<W: std::io::Write>(w: &mut Writer<W>, cp: &CharProps) {
    let has_content = cp.bold.is_some()
        || cp.italic.is_some()
        || cp.underline.is_some()
        || cp.strikethrough.is_some()
        || cp.font_size.is_some()
        || cp.font_name.is_some()
        || cp.color.is_some()
        || cp.background_color.is_some()
        || cp.small_caps.is_some()
        || cp.vertical_align.is_some();
    if !has_content {
        return;
    }
    let _ = write_start(w, "w:rPr", &[]);
    emit_char_props(w, cp);
    let _ = write_end(w, "w:rPr");
}

/// Emits `<w:rPr>` child elements for the given [`CharProps`] without the
/// wrapping `<w:rPr>` tags (so callers can embed additional children).
pub(super) fn emit_char_props<W: std::io::Write>(w: &mut Writer<W>, cp: &CharProps) {
    if let Some(ref font) = cp.font_name {
        let _ = write_empty(
            w,
            "w:rFonts",
            &[("w:ascii", font.as_str()), ("w:hAnsi", font.as_str())],
        );
    }
    if cp.bold == Some(true) {
        let _ = write_empty(w, "w:b", &[]);
    }
    if cp.italic == Some(true) {
        let _ = write_empty(w, "w:i", &[]);
    }
    if cp.small_caps == Some(true) {
        let _ = write_empty(w, "w:smallCaps", &[]);
    }
    if let Some(ref ul) = cp.underline {
        use loki_doc_model::style::props::char_props::UnderlineStyle;
        let v = match ul {
            UnderlineStyle::Double => "double",
            UnderlineStyle::Dotted => "dotted",
            UnderlineStyle::Dash => "dash",
            UnderlineStyle::Wave => "wave",
            UnderlineStyle::Thick => "thick",
            _ => "single",
        };
        let _ = write_empty(w, "w:u", &wval(v));
    }
    if let Some(ref st) = cp.strikethrough {
        use loki_doc_model::style::props::char_props::StrikethroughStyle;
        match st {
            StrikethroughStyle::Double => {
                let _ = write_empty(w, "w:dstrike", &[]);
            }
            _ => {
                let _ = write_empty(w, "w:strike", &[]);
            }
        }
    }
    if let Some(ref va) = cp.vertical_align {
        use loki_doc_model::style::props::char_props::VerticalAlign;
        let v = match va {
            VerticalAlign::Superscript => "superscript",
            VerticalAlign::Subscript => "subscript",
            _ => "baseline",
        };
        let _ = write_empty(w, "w:vertAlign", &wval(v));
    }
    if let Some(ref color) = cp.color
        && let Some(hex) = color.to_hex()
    {
        let hex_val = hex_color_val(&hex);
        let _ = write_empty(w, "w:color", &wval(&hex_val));
    }
    if let Some(pt) = cp.font_size {
        let half = pts_to_half_pts(pt.value()).to_string();
        let _ = write_empty(w, "w:sz", &wval(&half));
        let _ = write_empty(w, "w:szCs", &wval(&half));
    }
}
