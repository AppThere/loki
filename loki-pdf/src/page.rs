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
    DecorationKind, GlyphEntry, LayoutPage, LayoutRect, PositionedDecoration, PositionedGlyphRun,
    PositionedImage, PositionedItem,
};
use pdf_writer::{Content, Name, Str};

use crate::color::{Cmyk, layout_to_cmyk};
use crate::fonts::FontBank;
use crate::image::ImageBank;

/// The set of banks a page's content draws into while it is rendered.
pub struct PageBanks<'a> {
    /// Font faces and the glyphs they use.
    pub fonts: &'a mut FontBank,
    /// Decoded, CMYK image XObjects.
    pub images: &'a mut ImageBank,
}

/// Builds the content-stream bytes for `page`, registering every glyph run's
/// face and every image into `banks`.
pub fn render_page_content(page: &LayoutPage, banks: &mut PageBanks) -> Vec<u8> {
    let height = page.page_size.height;
    let mut content = Content::new();

    let (mx, my) = (page.margins.left, page.margins.top);
    for item in &page.content_items {
        render_item(item, height, mx, my, banks, &mut content);
    }
    for item in page.header_items.iter().chain(page.footer_items.iter()) {
        render_item(item, height, 0.0, 0.0, banks, &mut content);
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
    banks: &mut PageBanks,
    content: &mut Content,
) {
    match item {
        PositionedItem::GlyphRun(run) => render_run(run, page_h, ox, oy, banks.fonts, content),
        PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => {
            fill_rect(&r.rect, layout_to_cmyk(r.color), page_h, ox, oy, content);
        }
        PositionedItem::Decoration(d) => render_decoration(d, page_h, ox, oy, content),
        PositionedItem::BorderRect(b) => render_border(b, page_h, ox, oy, content),
        PositionedItem::Image(img) => draw_image(img, page_h, ox, oy, banks.images, content),
        PositionedItem::ClippedGroup { clip_rect, items } => {
            // Clip children to `clip_rect` (page-content-local coords). Used for
            // page-fragment masks and table cell boxes so over-wide content does
            // not bleed past its region — matching Word and the loki-vello
            // on-screen renderer. PDF clips with `re W n`: define the rect, set
            // it as the clip path, then end the path without painting it.
            let x = ox + clip_rect.origin.x;
            // PDF y-axis is bottom-up; flip the rect's top-left to bottom-left.
            let y = page_h - (oy + clip_rect.origin.y + clip_rect.size.height);
            content.save_state();
            content.rect(x, y, clip_rect.size.width, clip_rect.size.height);
            content.clip_nonzero();
            content.end_path();
            for child in items {
                render_item(child, page_h, ox, oy, banks, content);
            }
            content.restore_state();
        }
        PositionedItem::RotatedGroup { origin, items, .. } => {
            // TODO(pdf-rotate): rotation transform is not yet emitted; render
            // children at the group origin without rotation.
            for child in items {
                render_item(child, page_h, ox + origin.x, oy + origin.y, banks, content);
            }
        }
        _ => {}
    }
}

/// Draws an image by registering it in the bank and painting its XObject scaled
/// into the image rect. PDF image space is a unit square, so the CTM maps it to
/// the destination rectangle (origin bottom-left after the y-flip).
fn draw_image(
    img: &PositionedImage,
    page_h: f32,
    ox: f32,
    oy: f32,
    images: &mut ImageBank,
    content: &mut Content,
) {
    let Some(resource) = images.use_image(&img.src) else {
        return;
    };
    let w = img.rect.size.width;
    let h = img.rect.size.height;
    let x = ox + img.rect.origin.x;
    let y = page_h - (oy + img.rect.origin.y + h);
    content.save_state();
    content.transform([w, 0.0, 0.0, h, x, y]);
    content.x_object(Name(resource.as_bytes()));
    content.restore_state();
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
    // Glyph id 0 is the .notdef glyph (rendered as a tofu box by most fonts).
    // Skip it so characters with no font coverage are invisible, matching Word
    // and the on-screen `loki-vello` renderer (which filters id 0 identically).
    // Notably this drops the `.notdef` glyph that Parley shapes for tab
    // characters (`\t`); the tab's advance is already provided by the layout's
    // tab-stop inline box, so only the spurious tofu ink is removed.
    let drawn: Vec<&GlyphEntry> = run.glyphs.iter().filter(|g| g.id != 0).collect();
    if drawn.is_empty() {
        return;
    }
    let resource = bank.use_face(&run.font_data, run.font_index, drawn.iter().map(|g| g.id));

    let cmyk: Cmyk = layout_to_cmyk(run.color);
    content.set_fill_cmyk(cmyk.c, cmyk.m, cmyk.y, cmyk.k);
    content.begin_text();
    content.set_font(pdf_writer::Name(resource.as_bytes()), run.font_size);
    for glyph in drawn {
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

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
