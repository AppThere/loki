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
pub use flow::{flow_section, FlowOutput, LayoutWarning};
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
                let page_size = LayoutSize::new(
                    pts_to_f32(pl.page_size.width),
                    pts_to_f32(pl.page_size.height),
                );
                if first_page_size.is_none() {
                    first_page_size = Some(page_size);
                }

                // flow_section builds LayoutPage objects directly; use them
                // as-is (no re-binning). This fixes the margins.top offset
                // bug described in ADR 004 §Context B.3.
                let FlowOutput::Pages { mut pages, .. } =
                    flow_section(resources, section, &doc.styles, &mode, display_scale)
                else {
                    unreachable!("flow_section in Paginated mode always returns Pages");
                };

                let section_page_count = pages.len();
                for page in &mut pages {
                    page.page_number = global_page_count + page.page_number;
                }
                all_pages.extend(pages);
                global_page_count += section_page_count;
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
                let FlowOutput::Canvas { mut items, height, .. } =
                    flow_section(resources, section, &doc.styles, &mode, display_scale)
                else {
                    unreachable!("flow_section in non-paginated mode always returns Canvas");
                };
                // Offset section items by the height of all preceding sections.
                for item in &mut items {
                    item.translate(0.0, total_height);
                }
                all_items.extend(items);
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
