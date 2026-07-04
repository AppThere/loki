// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `<w:rPr>` run-property serializer shared by the styles writer and the
//! document-body writer.
//!
//! The emitter is kept symmetric with the reader (`docx/reader/document.rs`):
//! every run property the importer parses from `w:rPr` is written back here, so
//! direct character formatting survives an export→re-import round-trip.
//!
//! ECMA-376 §17.3.2 (Run properties).

use quick_xml::Writer;

use loki_doc_model::style::props::char_props::{CharProps, HighlightColor};

use crate::docx::write::xml::{
    hex_color_val, pts_to_half_pts, pts_to_twips, write_empty, write_end, write_start, wval,
};

/// Writes a `<w:rPr>` element from [`CharProps`] (nothing if no field is set).
pub(crate) fn write_char_props_elem<W: std::io::Write>(w: &mut Writer<W>, cp: &CharProps) {
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
pub(crate) fn emit_char_props<W: std::io::Write>(w: &mut Writer<W>, cp: &CharProps) {
    // A single `<w:rFonts>` carries the ascii/hAnsi, complex-script (cs), and
    // East-Asian (eastAsia) faces — symmetric with the reader, which parses all
    // three attributes from one element.
    if cp.font_name.is_some() || cp.font_name_complex.is_some() || cp.font_name_east_asian.is_some()
    {
        let mut attrs: Vec<(&str, &str)> = Vec::new();
        if let Some(ref font) = cp.font_name {
            attrs.push(("w:ascii", font.as_str()));
            attrs.push(("w:hAnsi", font.as_str()));
        }
        if let Some(ref font) = cp.font_name_complex {
            attrs.push(("w:cs", font.as_str()));
        }
        if let Some(ref font) = cp.font_name_east_asian {
            attrs.push(("w:eastAsia", font.as_str()));
        }
        let _ = write_empty(w, "w:rFonts", &attrs);
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
    if cp.all_caps == Some(true) {
        let _ = write_empty(w, "w:caps", &[]);
    }
    if cp.shadow == Some(true) {
        let _ = write_empty(w, "w:shadow", &[]);
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
    }
    // Complex-script size is independent of `w:sz`; emit it from its own field
    // (falling back to `w:sz` is the reader's job, not the writer's).
    if let Some(pt) = cp.font_size_complex {
        let half = pts_to_half_pts(pt.value()).to_string();
        let _ = write_empty(w, "w:szCs", &wval(&half));
    }
    if let Some(h) = cp.highlight_color
        && let Some(name) = highlight_val(h)
    {
        let _ = write_empty(w, "w:highlight", &wval(name));
    }
    if let Some(ref bg) = cp.background_color
        && let Some(hex) = bg.to_hex()
    {
        // `w:val="clear"` with a fill round-trips through `resolve_shading`.
        let fill = hex_color_val(&hex);
        let _ = write_empty(w, "w:shd", &[("w:val", "clear"), ("w:fill", &fill)]);
    }
    // Letter spacing is stored in points; OOXML `w:spacing @w:val` is twips.
    if let Some(ls) = cp.letter_spacing {
        let twips = pts_to_twips(ls.value()).to_string();
        let _ = write_empty(w, "w:spacing", &wval(&twips));
    }
    // Horizontal scale: model fraction (1.0 = 100%) → integer percent.
    if let Some(scale) = cp.scale {
        #[allow(clippy::cast_possible_truncation)] // bounded document measurement
        let pct = (scale * 100.0).round() as i32;
        let _ = write_empty(w, "w:w", &wval(&pct.to_string()));
    }
    // Kerning is a bool in the model; emit a non-zero half-point threshold when
    // enabled (the reader maps any positive value back to `true`).
    if let Some(kern) = cp.kerning {
        let v = if kern { "2" } else { "0" };
        let _ = write_empty(w, "w:kern", &wval(v));
    }
    if cp.language.is_some() || cp.language_complex.is_some() || cp.language_east_asian.is_some() {
        let mut attrs: Vec<(&str, &str)> = Vec::new();
        if let Some(ref l) = cp.language {
            attrs.push(("w:val", l.as_str()));
        }
        if let Some(ref l) = cp.language_complex {
            attrs.push(("w:bidi", l.as_str()));
        }
        if let Some(ref l) = cp.language_east_asian {
            attrs.push(("w:eastAsia", l.as_str()));
        }
        let _ = write_empty(w, "w:lang", &attrs);
    }
}

/// Reverse of the import `map_highlight`: a [`HighlightColor`] to its OOXML
/// `w:highlight @w:val` token. `None` (no highlight) yields `None`.
fn highlight_val(h: HighlightColor) -> Option<&'static str> {
    use HighlightColor as H;
    Some(match h {
        H::Black => "black",
        H::Blue => "blue",
        H::Cyan => "cyan",
        H::DarkBlue => "darkBlue",
        H::DarkCyan => "darkCyan",
        H::DarkGray => "darkGray",
        H::DarkGreen => "darkGreen",
        H::DarkMagenta => "darkMagenta",
        H::DarkRed => "darkRed",
        H::DarkYellow => "darkYellow",
        H::Green => "green",
        H::LightGray => "lightGray",
        H::Magenta => "magenta",
        H::Red => "red",
        H::White => "white",
        H::Yellow => "yellow",
        // `H::None` (no highlight) and any future `#[non_exhaustive]` variant
        // emit nothing rather than a wrong colour.
        _ => return None,
    })
}

#[cfg(test)]
#[path = "run_props_tests.rs"]
mod tests;
