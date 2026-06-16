// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Renders one laid-out page into a PDF content stream.
//!
//! Layout space is points, y-down, origin at the page top-left. PDF space is
//! points, y-up, origin at the page bottom-left, so every y coordinate is
//! flipped through the page height. Text and graphics are emitted in
//! DeviceCMYK (see [`crate::color`]); glyphs are addressed by raw id against
//! the embedded `Identity-H` fonts collected in the [`FontBank`].

use loki_layout::{
    DecorationKind, LayoutPage, LayoutRect, PositionedDecoration, PositionedGlyphRun,
    PositionedItem,
};
use pdf_writer::{Content, Str};

use crate::color::{Cmyk, layout_to_cmyk};
use crate::fonts::FontBank;

/// Builds the content-stream bytes for `page`, registering every glyph run's
/// face and used glyphs into `bank`.
pub fn render_page_content(page: &LayoutPage, bank: &mut FontBank) -> Vec<u8> {
    let height = page.page_size.height;
    let mut content = Content::new();

    let (mx, my) = (page.margins.left, page.margins.top);
    for item in &page.content_items {
        render_item(item, height, mx, my, bank, &mut content);
    }
    for item in page.header_items.iter().chain(page.footer_items.iter()) {
        render_item(item, height, 0.0, 0.0, bank, &mut content);
    }
    content.finish().to_vec()
}

/// Renders a single item. `(ox, oy)` is the area offset (margins for content
/// items, zero for header/footer) added to every layout coordinate.
fn render_item(
    item: &PositionedItem,
    page_h: f32,
    ox: f32,
    oy: f32,
    bank: &mut FontBank,
    content: &mut Content,
) {
    match item {
        PositionedItem::GlyphRun(run) => render_run(run, page_h, ox, oy, bank, content),
        PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => {
            fill_rect(&r.rect, layout_to_cmyk(r.color), page_h, ox, oy, content);
        }
        PositionedItem::Decoration(d) => render_decoration(d, page_h, ox, oy, content),
        PositionedItem::BorderRect(b) => render_border(b, page_h, ox, oy, content),
        PositionedItem::ClippedGroup { items, .. } => {
            // TODO(pdf-clip): clipping is not yet emitted; render children so
            // no content is dropped (over-paint is preferable to omission).
            for child in items {
                render_item(child, page_h, ox, oy, bank, content);
            }
        }
        PositionedItem::RotatedGroup { origin, items, .. } => {
            // TODO(pdf-rotate): rotation transform is not yet emitted; render
            // children at the group origin without rotation.
            for child in items {
                render_item(child, page_h, ox + origin.x, oy + origin.y, bank, content);
            }
        }
        // Images are not yet embedded (see crate docs).
        PositionedItem::Image(_) => {}
        _ => {}
    }
}

fn render_run(
    run: &PositionedGlyphRun,
    page_h: f32,
    ox: f32,
    oy: f32,
    bank: &mut FontBank,
    content: &mut Content,
) {
    if run.glyphs.is_empty() {
        return;
    }
    let resource = bank.use_face(
        &run.font_data,
        run.font_index,
        run.glyphs.iter().map(|g| g.id),
    );

    let cmyk: Cmyk = layout_to_cmyk(run.color);
    content.set_fill_cmyk(cmyk.c, cmyk.m, cmyk.y, cmyk.k);
    content.begin_text();
    content.set_font(pdf_writer::Name(resource.as_bytes()), run.font_size);
    for glyph in &run.glyphs {
        let x = ox + run.origin.x + glyph.x;
        let baseline = oy + run.origin.y + glyph.y;
        let y = page_h - baseline;
        content.set_text_matrix([1.0, 0.0, 0.0, 1.0, x, y]);
        let bytes = [(glyph.id >> 8) as u8, (glyph.id & 0xff) as u8];
        content.show(Str(&bytes));
    }
    content.end_text();
}

fn fill_rect(rect: &LayoutRect, color: Cmyk, page_h: f32, ox: f32, oy: f32, content: &mut Content) {
    let x = ox + rect.origin.x;
    let w = rect.size.width;
    let h = rect.size.height;
    let y = page_h - (oy + rect.origin.y + h);
    content.set_fill_cmyk(color.c, color.m, color.y, color.k);
    content.rect(x, y, w, h);
    content.fill_nonzero();
}

fn render_decoration(
    d: &PositionedDecoration,
    page_h: f32,
    ox: f32,
    oy: f32,
    content: &mut Content,
) {
    // Decorations are drawn as thin filled rectangles so the colour pipeline
    // stays fill-only (no stroke colour space needed).
    let thickness = d.thickness.max(0.5);
    let y_top = match d.kind {
        // `d.y` is the baseline; underline sits just below it.
        DecorationKind::Underline => d.y + thickness,
        DecorationKind::Strikethrough | DecorationKind::Overline => d.y - thickness,
        // `DecorationKind` is non-exhaustive; treat unknown kinds as overline.
        _ => d.y - thickness,
    };
    let rect = LayoutRect::new(d.x, y_top, d.width, thickness);
    fill_rect(&rect, layout_to_cmyk(d.color), page_h, ox, oy, content);
}

fn render_border(
    b: &loki_layout::PositionedBorderRect,
    page_h: f32,
    ox: f32,
    oy: f32,
    content: &mut Content,
) {
    let r = &b.rect;
    let (x0, y0, w, h) = (r.origin.x, r.origin.y, r.size.width, r.size.height);
    let mut edge = |rect: LayoutRect, color| fill_rect(&rect, color, page_h, ox, oy, content);
    if let Some(t) = &b.top {
        edge(LayoutRect::new(x0, y0, w, t.width), layout_to_cmyk(t.color));
    }
    if let Some(bottom) = &b.bottom {
        edge(
            LayoutRect::new(x0, y0 + h - bottom.width, w, bottom.width),
            layout_to_cmyk(bottom.color),
        );
    }
    if let Some(l) = &b.left {
        edge(LayoutRect::new(x0, y0, l.width, h), layout_to_cmyk(l.color));
    }
    if let Some(right) = &b.right {
        edge(
            LayoutRect::new(x0 + w - right.width, y0, right.width, h),
            layout_to_cmyk(right.color),
        );
    }
}
