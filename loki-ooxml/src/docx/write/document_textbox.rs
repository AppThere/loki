// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Floating `wps` text-box export (`Inline::TextBox`), split from
//! `document_inlines.rs`/`document_drawing.rs` for the 300-line ceiling.
//!
//! Writes a `w:drawing`/`wp:anchor` whose `a:graphicData` is a
//! `wps:wsp` shape carrying a `w:txbxContent` body — the reverse of the
//! reader's `parse_txbx_content` (`docx/reader/document_drawing.rs`), so an
//! imported text box round-trips: geometry (`wp:extent`), wrap
//! (`wp:wrapSquare`/…), the shape fill + border (`a:solidFill` / `a:ln`), and
//! the interior block content all survive an export→re-import.

use quick_xml::Writer;
use quick_xml::events::{BytesText, Event};

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::float::FloatWrap;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::xml::{NS_A, NS_WPS, write_empty, write_end, write_start};

use super::drawing::write_wrap_element;
use super::write_blocks;

// The `a:graphicData/@uri` identifying a WordprocessingShape (the DrawingML
// content type the reader keys `wps:txbx` off) is the same URI as the `wps`
// namespace, so `NS_WPS` serves both.

/// Reads a `u64` value stored on `attr.kv` under `key` (geometry EMU), or `0`.
fn kv_u64(attr: &NodeAttr, key: &str) -> u64 {
    attr.kv
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0)
}

/// Reads a `&str` value stored on `attr.kv` under `key`.
fn kv_str<'a>(attr: &'a NodeAttr, key: &str) -> Option<&'a str> {
    attr.kv
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Emits an [`Inline::TextBox`] as an anchored `wps` shape drawing.
///
/// Position is not modelled (as with [`super::drawing::write_anchor_drawing`]) —
/// the anchor is written at a zero column/paragraph offset and the reader
/// recovers only the wrap; re-import reconstructs the same box.
pub(super) fn write_textbox_drawing<W: std::io::Write>(
    w: &mut Writer<W>,
    attr: &NodeAttr,
    blocks: &[Block],
    collector: &mut ExportCollector,
) {
    let cx = kv_u64(attr, "cx_emu").to_string();
    let cy = kv_u64(attr, "cy_emu").to_string();
    let wrap = FloatWrap::read_or_class_default(attr).unwrap_or_default();
    let behind = if wrap.behind_text { "1" } else { "0" };

    let _ = write_start(w, "w:r", &[]);
    let _ = write_start(w, "w:drawing", &[]);
    let _ = write_start(
        w,
        "wp:anchor",
        &[
            ("distT", "0"),
            ("distB", "0"),
            ("distL", "0"),
            ("distR", "0"),
            ("simplePos", "0"),
            ("relativeHeight", "0"),
            ("behindDoc", behind),
            ("locked", "0"),
            ("layoutInCell", "1"),
            ("allowOverlap", "1"),
        ],
    );
    let _ = write_empty(w, "wp:simplePos", &[("x", "0"), ("y", "0")]);
    write_zero_position(w, "wp:positionH", "column");
    write_zero_position(w, "wp:positionV", "paragraph");
    let _ = write_empty(w, "wp:extent", &[("cx", &cx), ("cy", &cy)]);
    write_wrap_element(w, wrap);
    let _ = write_empty(w, "wp:docPr", &[("id", "1"), ("name", "TextBox")]);

    write_wsp(w, attr, blocks, collector);

    let _ = write_end(w, "wp:anchor");
    let _ = write_end(w, "w:drawing");
    let _ = write_end(w, "w:r");
}

/// Writes a `wp:positionH`/`positionV` at a zero offset relative to `rel`.
fn write_zero_position<W: std::io::Write>(w: &mut Writer<W>, tag: &str, rel: &str) {
    let _ = write_start(w, tag, &[("relativeFrom", rel)]);
    let _ = write_start(w, "wp:posOffset", &[]);
    let _ = w.write_event(Event::Text(BytesText::new("0")));
    let _ = write_end(w, "wp:posOffset");
    let _ = write_end(w, tag);
}

/// Emits the `a:graphic`/`a:graphicData`/`wps:wsp` body: the shape properties
/// (fill + border) and the `w:txbxContent` interior blocks.
fn write_wsp<W: std::io::Write>(
    w: &mut Writer<W>,
    attr: &NodeAttr,
    blocks: &[Block],
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "a:graphic", &[("xmlns:a", NS_A)]);
    let _ = write_start(w, "a:graphicData", &[("uri", NS_WPS)]);
    let _ = write_start(w, "wps:wsp", &[("xmlns:wps", NS_WPS)]);

    let _ = write_start(w, "wps:spPr", &[]);
    // Fill `a:srgbClr` must precede the `a:ln` border so the reader (which keys
    // the first non-`ln` `srgbClr` as the fill) recovers each colour correctly.
    if let Some(fill) = kv_str(attr, "textbox-fill") {
        let _ = write_start(w, "a:solidFill", &[]);
        let _ = write_empty(w, "a:srgbClr", &[("val", fill)]);
        let _ = write_end(w, "a:solidFill");
    }
    if let Some(line) = kv_str(attr, "textbox-line") {
        // 1 pt border = 12700 EMU; matches the reader's `a:ln/@w` parse.
        let _ = write_start(w, "a:ln", &[("w", "12700")]);
        let _ = write_start(w, "a:solidFill", &[]);
        let _ = write_empty(w, "a:srgbClr", &[("val", line)]);
        let _ = write_end(w, "a:solidFill");
        let _ = write_end(w, "a:ln");
    }
    let _ = write_end(w, "wps:spPr");

    let _ = write_start(w, "wps:txbx", &[]);
    let _ = write_start(w, "w:txbxContent", &[]);
    write_blocks(w, blocks, collector, 0);
    let _ = write_end(w, "w:txbxContent");
    let _ = write_end(w, "wps:txbx");

    let _ = write_empty(w, "wps:bodyPr", &[]);

    let _ = write_end(w, "wps:wsp");
    let _ = write_end(w, "a:graphicData");
    let _ = write_end(w, "a:graphic");
}
