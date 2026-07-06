// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `word/styles.xml` serializer.
//!
//! Emits `docDefaults`, the `Normal` paragraph style, `Heading1`–`Heading6`,
//! and all named paragraph/character styles from the document's
//! [`StyleCatalog`]. When the catalog already carries `Normal` or a heading
//! style (e.g. after import or a style-editor edit), its full properties are
//! emitted so they round-trip; otherwise a built-in default is synthesized.
//!
//! ECMA-376 §17.7 (Document Styles).

use quick_xml::Writer;

use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::para_style::ParagraphStyle;

use crate::docx::write::run_props::write_char_props_elem;
use crate::docx::write::style_props::write_para_props_elem;
use crate::docx::write::xml::{NS_W, write_decl, write_empty, write_end, write_start, wval};

/// Built-in heading definitions: (styleId, display name, font size half-pts, bold, outline level).
const HEADINGS: &[(&str, &str, i32, bool, u8)] = &[
    ("Heading1", "heading 1", 48, true, 0),
    ("Heading2", "heading 2", 36, true, 1),
    ("Heading3", "heading 3", 28, true, 2),
    ("Heading4", "heading 4", 24, true, 3),
    ("Heading5", "heading 5", 24, false, 4),
    ("Heading6", "heading 6", 20, false, 5),
];

/// Returns `true` if `sid` is `Normal` or one of the built-in heading ids.
fn is_builtin_id(sid: &str) -> bool {
    sid == "Normal" || HEADINGS.iter().any(|(h, _, _, _, _)| *h == sid)
}

/// Synthetic internal styles (id prefixed `__`, e.g. `__DocDefault` /
/// `__DocDefaultChar`) represent the document's `docDefaults`, not user-named
/// styles — they are emitted as `w:docDefaults`, never as `w:style` entries.
fn is_synthetic_id(sid: &str) -> bool {
    sid.starts_with("__")
}

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

    // Normal: emit the catalog's version (with its edited props) when present,
    // else a minimal default flagged as the document default.
    match catalog.paragraph_styles.get(&StyleId::new("Normal")) {
        Some(style) => emit_paragraph_style(&mut w, style),
        None => write_default_normal(&mut w, catalog),
    }

    // Headings: emit the catalog's version when present (so style-editor edits
    // to a heading persist), else the built-in default.
    for &(style_id, name, sz_half_pts, bold, outline_lvl) in HEADINGS {
        match catalog.paragraph_styles.get(&StyleId::new(style_id)) {
            Some(style) => emit_paragraph_style(&mut w, style),
            None => write_default_heading(&mut w, style_id, name, sz_half_pts, bold, outline_lvl),
        }
    }

    // Remaining (custom / non-built-in) paragraph styles. Synthetic internal
    // styles (`__`-prefixed, e.g. `__DocDefault` — the docDefaults source) are
    // not real named styles and are written as `w:docDefaults`, not `w:style`.
    for (id, style) in &catalog.paragraph_styles {
        if !is_builtin_id(id.as_str()) && !is_synthetic_id(id.as_str()) {
            emit_paragraph_style(&mut w, style);
        }
    }

    // Character styles (skip the synthetic `__DocDefaultChar` docDefaults source).
    for (id, style) in &catalog.character_styles {
        let sid = id.as_str();
        if is_synthetic_id(sid) {
            continue;
        }
        let _ = write_start(
            &mut w,
            "w:style",
            &[("w:type", "character"), ("w:styleId", sid)],
        );
        let name = style.display_name.as_deref().unwrap_or(sid);
        let _ = write_empty(&mut w, "w:name", &wval(name));
        if let Some(ref parent) = style.parent {
            let _ = write_empty(&mut w, "w:basedOn", &wval(parent.as_str()));
        }
        write_char_props_elem(&mut w, &style.char_props);
        let _ = write_end(&mut w, "w:style");
    }

    // Built-in note-reference character styles. These carry the superscript for
    // footnote/endnote markers (the body writer references them by id rather
    // than emitting an explicit `<w:vertAlign>` on each marker run). Word always
    // includes them; emit defaults only when the catalog did not already define
    // them above, so an imported style with edited props still wins.
    if !catalog
        .character_styles
        .contains_key(&StyleId::new("FootnoteReference"))
    {
        write_note_reference_style(&mut w, "FootnoteReference", "footnote reference");
    }
    if !catalog
        .character_styles
        .contains_key(&StyleId::new("EndnoteReference"))
    {
        write_note_reference_style(&mut w, "EndnoteReference", "endnote reference");
    }

    let _ = write_end(&mut w, "w:styles");
    drop(w);
    out
}

/// Emits a complete `<w:style w:type="paragraph">` element from a catalog style,
/// including `basedOn`, `next`, `customStyle`, `link`, and full `pPr` / `rPr`.
fn emit_paragraph_style<W: std::io::Write>(w: &mut Writer<W>, style: &ParagraphStyle) {
    let sid = style.id.as_str();
    let default_s = if style.is_default { "1" } else { "0" };
    let mut attrs: Vec<(&str, &str)> = vec![
        ("w:type", "paragraph"),
        ("w:styleId", sid),
        ("w:default", default_s),
    ];
    if style.is_custom {
        attrs.push(("w:customStyle", "1"));
    }
    let _ = write_start(w, "w:style", &attrs);

    let name = style.display_name.as_deref().unwrap_or(sid);
    let _ = write_empty(w, "w:name", &wval(name));
    if let Some(ref parent) = style.parent {
        let _ = write_empty(w, "w:basedOn", &wval(parent.as_str()));
    }
    if let Some(ref next) = style.next_style_id {
        let _ = write_empty(w, "w:next", &wval(next));
    }
    if let Some(ref link) = style.linked_char_style {
        let _ = write_empty(w, "w:link", &wval(link.as_str()));
    }
    write_para_props_elem(w, &style.para_props);
    write_char_props_elem(w, &style.char_props);
    let _ = write_end(w, "w:style");
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

/// Minimal default `Normal` style (used when the catalog has no `Normal`).
fn write_default_normal<W: std::io::Write>(w: &mut Writer<W>, catalog: &StyleCatalog) {
    let is_default = catalog
        .paragraph_styles
        .get(&StyleId::new("Normal"))
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

/// Built-in default heading style (used when the catalog has no such heading).
fn write_default_heading<W: std::io::Write>(
    w: &mut Writer<W>,
    style_id: &str,
    name: &str,
    sz_half_pts: i32,
    bold: bool,
    outline_lvl: u8,
) {
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
    if bold {
        let _ = write_empty(w, "w:b", &[]);
    }
    let sz_s = sz_half_pts.to_string();
    let _ = write_empty(w, "w:sz", &wval(&sz_s));
    let _ = write_empty(w, "w:szCs", &wval(&sz_s));
    let _ = write_end(w, "w:rPr");

    let _ = write_end(w, "w:style");
}

/// Writes a built-in `<w:style w:type="character">` for a footnote/endnote
/// reference: only the superscript run property, matching Word's defaults.
fn write_note_reference_style<W: std::io::Write>(w: &mut Writer<W>, style_id: &str, name: &str) {
    let _ = write_start(
        w,
        "w:style",
        &[("w:type", "character"), ("w:styleId", style_id)],
    );
    let _ = write_empty(w, "w:name", &wval(name));
    let _ = write_start(w, "w:rPr", &[]);
    let _ = write_empty(w, "w:vertAlign", &wval("superscript"));
    let _ = write_end(w, "w:rPr");
    let _ = write_end(w, "w:style");
}
