// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Renderer-agnostic layout engine for the Loki suite.
//!
//! `loki-layout` takes a [`loki_doc_model::Document`] and produces a layout
//! result containing absolute positions for all content elements. It has no
//! GPU dependencies and is fully testable without a display.
//!
//! # Layout Modes
//!
//! Three modes are supported via [`LayoutMode`]:
//!
//! - [`LayoutMode::Paginated`]: content broken into fixed-size pages.
//! - [`LayoutMode::Pageless`]: single infinite canvas, document-width content.
//! - [`LayoutMode::Reflow`]: single infinite canvas, caller-supplied width.
//!
//! # Output
//!
//! Layout produces a [`DocumentLayout`] containing [`PositionedItem`]s, each
//! carrying absolute coordinates ready for a renderer such as `loki-vello`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod color;
pub mod error;
pub mod flow;
pub mod font;
pub mod geometry;
pub mod items;
pub mod mode;
pub mod para;
pub mod resolve;
pub mod result;

pub use color::LayoutColor;
pub use error::{LayoutError, LayoutResult};
pub use geometry::{LayoutInsets, LayoutPoint, LayoutRect, LayoutSize};
pub use items::{
    BorderEdge, BorderStyle, DecorationKind, GlyphEntry, GlyphSynthesis,
    PositionedBorderRect, PositionedDecoration, PositionedGlyphRun, PositionedImage,
    PositionedItem, PositionedRect,
};
pub use flow::{flow_section, LayoutWarning};
pub use font::FontResources;
pub use mode::LayoutMode;
pub use para::{layout_paragraph, ParagraphLayout, ResolvedLineHeight, ResolvedParaProps, StyleSpan};
pub use resolve::{flatten_paragraph, pts_to_f32, resolve_char_props, resolve_color, resolve_para_props};
pub use result::{ContinuousLayout, DocumentLayout, LayoutPage, PaginatedLayout};

/// Lays out a full document into absolute positions.
///
/// This processes all sections in the document and returns a single [`DocumentLayout`].
/// In the current implementation, sections are stacked vertically.
pub fn layout_document(
    resources: &mut FontResources,
    doc: &loki_doc_model::Document,
    mode: LayoutMode,
    display_scale: f32,
) -> DocumentLayout {
    match mode {
        LayoutMode::Paginated => {
            let mut all_pages = Vec::new();
            let mut global_page_count = 0;
            let mut first_page_size = None;

            for section in &doc.sections {
                let pl = &section.layout;
                let page_size = LayoutSize::new(pts_to_f32(pl.page_size.width), pts_to_f32(pl.page_size.height));
                if first_page_size.is_none() {
                    first_page_size = Some(page_size);
                }

                let margins = LayoutInsets {
                    top: pts_to_f32(pl.margins.top),
                    right: pts_to_f32(pl.margins.right),
                    bottom: pts_to_f32(pl.margins.bottom),
                    left: pts_to_f32(pl.margins.left),
                };

                // For now, we reuse flow_section and manually re-page the results.
                // This is a bit inefficient but works for v0.1.
                let (items, _, _) = flow_section(resources, section, &doc.styles, &mode, display_scale);
                
                let page_h = page_size.height;
                let page_count = (items.iter()
                    .filter_map(|i| {
                        match i {
                            PositionedItem::GlyphRun(r) => Some(r.origin.y),
                            PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => Some(r.rect.origin.y),
                            PositionedItem::BorderRect(r) => Some(r.rect.origin.y),
                            PositionedItem::Image(r) => Some(r.rect.origin.y),
                            PositionedItem::Decoration(d) => Some(d.y),
                        }
                    })
                    .fold(0.0f32, |m, y| m.max(y)) / page_h).ceil() as usize;
                
                let page_count = page_count.max(1);

                for p_idx in 0..page_count {
                    let mut page_items = Vec::new();
                    let page_top = p_idx as f32 * page_h;
                    let page_bottom = (p_idx + 1) as f32 * page_h;

                    for item in &items {
                        let y = match item {
                            PositionedItem::GlyphRun(r) => r.origin.y,
                            PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => r.rect.origin.y,
                            PositionedItem::BorderRect(r) => r.rect.origin.y,
                            PositionedItem::Image(r) => r.rect.origin.y,
                            PositionedItem::Decoration(d) => d.y,
                        };

                        if y >= page_top && y < page_bottom {
                            let mut it = item.clone();
                            // Translate to page-local
                            match &mut it {
                                PositionedItem::GlyphRun(r) => { r.origin.y -= page_top; }
                                PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => { r.rect.origin.y -= page_top; }
                                PositionedItem::BorderRect(r) => { r.rect.origin.y -= page_top; }
                                PositionedItem::Image(r) => { r.rect.origin.y -= page_top; }
                                PositionedItem::Decoration(d) => { d.y -= page_top; }
                            }
                            page_items.push(it);
                        }
                    }

                    all_pages.push(LayoutPage {
                        page_number: global_page_count + p_idx + 1,
                        page_size,
                        margins,
                        content_items: page_items,
                        header_items: vec![],
                        footer_items: vec![],
                    });
                }
                global_page_count += page_count;
            }

            DocumentLayout::Paginated(PaginatedLayout {
                page_size: first_page_size.unwrap_or_default(),
                pages: all_pages,
            })
        }
        _ => {
            let mut all_items = Vec::new();
            let mut total_height = 0.0;
            let mut max_width: f32 = 0.0;

            for section in &doc.sections {
                let (items, height, _) = flow_section(resources, section, &doc.styles, &mode, display_scale);
                for mut item in items {
                    // Offset section items by previous sections' height
                    match &mut item {
                        PositionedItem::GlyphRun(r) => r.origin.y += total_height,
                        PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => r.rect.origin.y += total_height,
                        PositionedItem::BorderRect(r) => r.rect.origin.y += total_height,
                        PositionedItem::Image(r) => r.rect.origin.y += total_height,
                        PositionedItem::Decoration(d) => d.y += total_height,
                    }
                    all_items.push(item);
                }
                total_height += height;
                
                let pl = &section.layout;
                let page_w = pts_to_f32(pl.page_size.width);
                max_width = max_width.max(page_w);
            }

            DocumentLayout::Continuous(ContinuousLayout {
                content_width: match mode {
                    LayoutMode::Reflow { available_width } => available_width,
                    _ => max_width,
                },
                total_height,
                items: all_items,
            })
        }
    }
}
