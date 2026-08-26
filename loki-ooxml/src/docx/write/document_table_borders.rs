// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX table **border** serialization: the table's own `w:tblBorders` and the
//! shared per-edge writer that `w:tcBorders` also uses.
//!
//! Split out of `write/document_table.rs` to keep it under the 300-line
//! ceiling. Both writers stay in one module because they share the
//! absent-vs-`nil` rule: an absent edge inherits, an explicit `nil` suppresses.

use quick_xml::Writer;

use crate::docx::write::xml::{color_to_hex, write_empty, write_end, write_start};

/// Writes the table's own `w:tblBorders` into `w:tblPr`, or nothing when the
/// table states no border set of its own (the style then decides).
///
/// Placed after `w:tblW` and before `w:tblLook` per `CT_TblPrBase`'s sequence.
/// Edges are serialized by the same helper `w:tcBorders` uses — the element
/// content model is identical, and sharing it keeps the absent-vs-`nil`
/// distinction (which decides whether an edge falls back to the style or
/// suppresses it) from being re-derived a second way on the way out.
pub(super) fn write_tbl_borders<W: std::io::Write>(
    w: &mut Writer<W>,
    borders: Option<&loki_doc_model::style::table_borders::TableBorders>,
) {
    let Some(b) = borders else { return };
    let _ = write_start(w, "w:tblBorders", &[]);
    for (tag, edge) in [
        ("w:top", &b.top),
        ("w:left", &b.left),
        ("w:bottom", &b.bottom),
        ("w:right", &b.right),
        ("w:insideH", &b.inside_h),
        ("w:insideV", &b.inside_v),
    ] {
        write_tc_border_edge(w, tag, edge.as_ref());
    }
    let _ = write_end(w, "w:tblBorders");
}

/// Writes one border edge from a model [`Border`]: style → `w:val`, width in
/// points → `w:sz` (eighth-points), colour → hex (or `auto`).
///
/// `BorderStyle::None` writes `w:val="nil"` — an explicit no-border, distinct
/// from an absent edge, which inherits instead. It takes the *same* attribute
/// path as every other style rather than a bare `w:val`: the attributes are
/// inert for an edge that draws nothing, but dropping them makes an explicit
/// no-border lossy across a round trip, and the `w:sz` clamp below would
/// promote it to a 0.25 pt hairline on the way back in.
pub(super) fn write_tc_border_edge<W: std::io::Write>(
    w: &mut Writer<W>,
    tag: &str,
    border: Option<&loki_doc_model::style::props::border::Border>,
) {
    use loki_doc_model::style::props::border::BorderStyle;
    let Some(b) = border else { return };
    let val = match b.style {
        BorderStyle::None => "nil",
        BorderStyle::Dashed => "dashed",
        BorderStyle::Dotted => "dotted",
        BorderStyle::Double => "double",
        BorderStyle::Inset => "inset",
        BorderStyle::Outset => "outset",
        BorderStyle::Wave => "wave",
        // Groove/Ridge have no OOXML equivalent (threeDEmboss/threeDEngrave
        // are visually different); Solid and future variants map to single.
        _ => "single",
    };
    // Eighth-points, clamped to OOXML's valid 2..=96 w:sz range — but only for
    // an edge that draws. A `nil` edge has no width to keep in range (Word
    // writes `w:sz="0"`), and clamping it up to 2 would turn "no border" into a
    // hairline the next time the file is opened.
    let lo = if b.style == BorderStyle::None {
        0.0
    } else {
        2.0
    };
    #[allow(clippy::cast_possible_truncation)] // clamped to lo..=96 above the cast
    let sz = ((b.width.value() * 8.0).round().clamp(lo, 96.0) as i32).to_string();
    let color = b
        .color
        .as_ref()
        .map_or_else(|| "auto".to_string(), color_to_hex);
    let _ = write_empty(
        w,
        tag,
        &[
            ("w:val", val),
            ("w:sz", &sz),
            ("w:space", "0"),
            ("w:color", &color),
        ],
    );
}
