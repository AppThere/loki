// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Floating text-box (`Inline::TextBox`) serialisation, split from `inlines.rs`
//! for the 300-line ceiling. Writes a `draw:frame`/`draw:text-box`.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::float::FloatWrap;

use super::super::content::{Cx, write_block};
use super::super::xml::attr;

/// Writes a floating `wps`/text-box (`Inline::TextBox`) as a
/// `<draw:frame><draw:text-box>…</draw:text-box></draw:frame>` anchored to the
/// paragraph, with an automatic graphic style carrying the wrap + fill + border.
/// The reverse of the importer's floating-text-box path (`mapper::…::frames`).
pub(super) fn write_text_box(
    out: &mut String,
    node_attr: &NodeAttr,
    blocks: &[Block],
    cx: &mut Cx,
) {
    let kv = |key: &str| {
        node_attr
            .kv
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    // EMU → pt (1 pt = 12700 EMU); the importer reads svg:width/height back as pt.
    let emu_pt = |key: &str| -> Option<String> {
        let emu: f64 = kv(key)?.parse().ok()?;
        (emu > 0.0).then(|| format!("{}pt", emu / 12700.0))
    };
    let wrap = FloatWrap::read_or_class_default(node_attr);
    let style = cx
        .auto
        .graphic_style(wrap, kv("textbox-fill"), kv("textbox-line"));

    out.push_str("<draw:frame");
    if let Some(name) = &style {
        attr(out, "draw:style-name", name);
    }
    attr(out, "text:anchor-type", "paragraph");
    if let Some(w) = emu_pt("cx_emu") {
        attr(out, "svg:width", &w);
    }
    if let Some(h) = emu_pt("cy_emu") {
        attr(out, "svg:height", &h);
    }
    out.push_str("><draw:text-box>");
    for block in blocks {
        write_block(out, block, cx);
    }
    out.push_str("</draw:text-box></draw:frame>");
}
