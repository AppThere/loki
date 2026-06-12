// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Render-side layout representation: paginated pages or reflow tiles.
//!
//! [`RenderLayout`] is what [`crate::DocPageSource`] caches and what the tile
//! components consume.  In paginated mode it wraps the print-fidelity
//! [`PaginatedLayout`] directly.  In reflow mode it wraps a
//! [`ContinuousLayout`] (the real layout engine run at the viewport width — see
//! `LayoutMode::Reflow`) and presents it as a sequence of fixed-height virtual
//! tiles so the existing page cache, scroll tiering, and texture machinery
//! work unchanged.  Tiles are stacked with zero gap; items spanning a tile
//! boundary are painted into both tiles (see `loki_vello::band`).

use std::sync::Arc;

use loki_layout::{ContinuousLayout, CursorRect, PaginatedLayout};
use loki_vello::{CursorPaint, FontDataCache, SelectionRect};

/// Height of one reflow render tile in layout points.  Chosen so the CSS-pixel
/// height is an exact integer (768 pt × 96/72 = 1024 px): fractional tile
/// heights leave a sub-pixel seam where stacked tiles meet.  Tall enough to
/// keep tile counts low, short enough that a Hot-tier texture (2× scale) stays
/// well inside common GPU texture limits.
pub const REFLOW_TILE_HEIGHT_PT: f32 = 768.0;

/// Horizontal reading-margin inset in points (24 CSS px each side).  The
/// reflow layout is computed at `tile width − 2 × inset` and painted shifted
/// right by this amount, so text never touches the viewport edge.
pub const REFLOW_PADDING_PT: f32 = 18.0;

/// Narrowest reflow content width the layout engine is asked for, in points.
/// Guards against degenerate layouts while the viewport width is still being
/// measured.
pub const MIN_REFLOW_CONTENT_PT: f32 = 50.0;

// ── RenderMode ────────────────────────────────────────────────────────────────

/// Which layout the renderer should produce.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode {
    /// Print-fidelity pages from the document's own page geometry.
    Paginated,
    /// Continuous web-style flow at the given available width (layout points,
    /// including the [`REFLOW_PADDING_PT`] insets on both sides).
    Reflow {
        /// Total tile width in points.
        available_width_pt: f32,
    },
}

impl RenderMode {
    /// Width-tolerant equality: reflow widths within half a point are treated
    /// as the same mode so sub-pixel viewport jitter does not trigger relayout.
    pub(crate) fn matches(&self, other: &RenderMode) -> bool {
        match (self, other) {
            (RenderMode::Paginated, RenderMode::Paginated) => true,
            (
                RenderMode::Reflow {
                    available_width_pt: a,
                },
                RenderMode::Reflow {
                    available_width_pt: b,
                },
            ) => (a - b).abs() < 0.5,
            _ => false,
        }
    }
}

// ── RenderLayout ──────────────────────────────────────────────────────────────

/// A computed layout in either rendering shape.
#[derive(Debug, Clone)]
pub enum RenderLayout {
    /// Paginated print layout — one tile per real page.
    ///
    /// Held behind an [`Arc`] so the editor's already-computed layout can be
    /// shared with the renderer without a deep clone (the single canonical
    /// layout — see [`crate::DocPageSource::provide_paginated_layout`]).
    Paginated(Arc<PaginatedLayout>),
    /// Continuous reflow layout — sliced into virtual tiles of
    /// [`REFLOW_TILE_HEIGHT_PT`].
    Reflow {
        /// The continuous layout (items in absolute canvas coordinates).
        layout: ContinuousLayout,
        /// Full tile width in points: `max(wrap width, widest content) + both
        /// padding insets`.  Wider than the viewport when content overflows
        /// (e.g. a fixed-width table), so it can be reached by horizontal
        /// scrolling instead of being clipped.
        tile_width_pt: f32,
    },
}

impl RenderLayout {
    /// Borrow the paginated layout, or `None` in reflow mode.  Callers that
    /// need per-page editing data (cursor painting, hit testing) use this and
    /// degrade gracefully in reflow mode, which carries no editing data.
    pub fn as_paginated(&self) -> Option<&PaginatedLayout> {
        match self {
            RenderLayout::Paginated(pl) => Some(pl.as_ref()),
            RenderLayout::Reflow { .. } => None,
        }
    }

    /// `true` when this is a reflow (zero-gap virtual tile) layout.
    pub fn is_reflow(&self) -> bool {
        matches!(self, RenderLayout::Reflow { .. })
    }

    /// Number of render tiles (pages, or reflow bands).
    pub fn page_count(&self) -> usize {
        match self {
            RenderLayout::Paginated(pl) => pl.pages.len(),
            RenderLayout::Reflow { layout, .. } => (layout.total_height / REFLOW_TILE_HEIGHT_PT)
                .ceil()
                .max(1.0) as usize,
        }
    }

    /// Size of one render tile in layout points, or `None` if out of range.
    pub fn page_size_pts(&self, index: usize) -> Option<(f32, f32)> {
        match self {
            RenderLayout::Paginated(pl) => pl
                .pages
                .get(index)
                .map(|p| (p.page_size.width, p.page_size.height)),
            RenderLayout::Reflow {
                layout,
                tile_width_pt,
            } => {
                if index >= self.page_count() {
                    return None;
                }
                let band_top = index as f32 * REFLOW_TILE_HEIGHT_PT;
                let height = (layout.total_height - band_top).clamp(1.0, REFLOW_TILE_HEIGHT_PT);
                Some((*tile_width_pt, height))
            }
        }
    }

    /// The continuous layout, or `None` in paginated mode.
    fn continuous(&self) -> Option<&ContinuousLayout> {
        match self {
            RenderLayout::Reflow { layout, .. } => Some(layout),
            RenderLayout::Paginated(_) => None,
        }
    }

    /// Hit-test a point in **canvas** coordinates (layout points, padding
    /// already removed) against the reflow layout, returning
    /// `(block_index, byte_offset)`.  `None` in paginated mode.
    pub fn reflow_hit_test(&self, canvas_x: f32, canvas_y: f32) -> Option<(usize, usize)> {
        self.continuous()?.hit_test(canvas_x, canvas_y)
    }

    /// Caret rectangle in **canvas** coordinates for `(block_index,
    /// byte_offset)`, or `None` in paginated mode / when not found.
    pub(crate) fn reflow_cursor_canvas(
        &self,
        block_index: usize,
        byte_offset: usize,
    ) -> Option<CursorRect> {
        self.continuous()?
            .cursor_rect_canvas(block_index, byte_offset)
    }

    /// Paint render tile `index` into `scene` at `scale`.
    ///
    /// Paginated tiles paint the full page (chrome, shadow, optional cursor via
    /// `cursor_paint`).  Reflow tiles paint the band of the continuous flow they
    /// cover, inset by [`REFLOW_PADDING_PT`]; the reflow caret is painted from
    /// `reflow_cursor` = `(block_index, byte_offset)` when it falls in the band.
    /// The reflow band has no explicit background — the caller's
    /// `RenderParams::base_color` (white) provides it.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_tile(
        &self,
        scene: &mut vello::Scene,
        font_cache: &mut FontDataCache,
        index: usize,
        scale: f32,
        cursor_paint: Option<&CursorPaint>,
        reflow_cursor: Option<(usize, usize)>,
        reflow_selection: Option<((usize, usize), (usize, usize))>,
    ) {
        match self {
            RenderLayout::Paginated(pl) => {
                loki_vello::paint_single_page(
                    scene,
                    pl,
                    font_cache,
                    (0.0, 0.0),
                    scale,
                    index,
                    cursor_paint,
                );
            }
            RenderLayout::Reflow { layout, .. } => {
                let band_top = index as f32 * REFLOW_TILE_HEIGHT_PT;
                let band_h = self
                    .page_size_pts(index)
                    .map(|(_, h)| h)
                    .unwrap_or(REFLOW_TILE_HEIGHT_PT);
                let offset = (REFLOW_PADDING_PT, -band_top);

                // Selection highlight first, so glyphs and the caret sit on top.
                // Clip to rects that intersect this band; paint_cursor draws the
                // selection rects (cursor_rect height 0 ⇒ no caret here).
                if let Some((a, b)) = reflow_selection {
                    let sel: Vec<SelectionRect> = layout
                        .selection_rects(a, b)
                        .into_iter()
                        .filter(|r| {
                            r.origin.y + r.size.height >= band_top
                                && r.origin.y <= band_top + band_h
                        })
                        .map(|r| SelectionRect {
                            x: r.origin.x,
                            y: r.origin.y,
                            width: r.size.width,
                            height: r.size.height,
                        })
                        .collect();
                    if !sel.is_empty() {
                        loki_vello::paint_cursor(
                            scene,
                            &CursorRect {
                                x: 0.0,
                                y: 0.0,
                                height: 0.0,
                            },
                            &sel,
                            &[],
                            offset,
                            scale,
                        );
                    }
                }

                loki_vello::paint_continuous_band(
                    scene,
                    layout,
                    font_cache,
                    REFLOW_PADDING_PT,
                    scale,
                    band_top,
                    band_h,
                );

                // Caret: paint when its canvas rect intersects this band,
                // translated into band-local space (same offset as the items).
                if let Some((para, byte)) = reflow_cursor
                    && let Some(cr) = self.reflow_cursor_canvas(para, byte)
                    && cr.y + cr.height >= band_top
                    && cr.y <= band_top + band_h
                {
                    loki_vello::paint_cursor(scene, &cr, &[], &[], offset, scale);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_layout::ContinuousLayout;

    fn reflow(total_height: f32, tile_width_pt: f32) -> RenderLayout {
        RenderLayout::Reflow {
            layout: ContinuousLayout {
                content_width: 0.0,
                total_height,
                items: vec![],
                paragraphs: vec![],
            },
            tile_width_pt,
        }
    }

    #[test]
    fn reflow_tile_count_and_sizes() {
        // 768 * 2 + 100 = 1636 → 3 tiles (two full, one 100pt remainder).
        let rl = reflow(1636.0, 500.0);
        assert_eq!(rl.page_count(), 3);
        assert_eq!(rl.page_size_pts(0), Some((500.0, 768.0)));
        assert_eq!(rl.page_size_pts(1), Some((500.0, 768.0)));
        assert_eq!(rl.page_size_pts(2), Some((500.0, 100.0)));
        assert_eq!(rl.page_size_pts(3), None);
        assert!(rl.is_reflow());
        assert!(rl.as_paginated().is_none());
    }

    #[test]
    fn reflow_always_has_at_least_one_tile() {
        let rl = reflow(0.0, 400.0);
        assert_eq!(rl.page_count(), 1);
        assert_eq!(rl.page_size_pts(0), Some((400.0, 1.0)));
    }

    fn one_para_reflow(text: &str, origin: (f32, f32)) -> RenderLayout {
        use loki_layout::{
            FontResources, LayoutColor, PageParagraphData, ResolvedParaProps, StyleSpan,
            layout_paragraph,
        };
        let mut resources = FontResources::new();
        let para = layout_paragraph(
            &mut resources,
            text,
            &[StyleSpan {
                range: 0..text.len(),
                font_name: None,
                font_size: 12.0,
                bold: false,
                italic: false,
                color: LayoutColor::BLACK,
                underline: None,
                strikethrough: None,
                line_height: None,
                vertical_align: None,
                highlight_color: None,
                letter_spacing: None,
                font_variant: None,
                word_spacing: None,
                shadow: false,
                link_url: None,
            }],
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        );
        let height = para.height;
        RenderLayout::Reflow {
            layout: ContinuousLayout {
                content_width: 400.0,
                total_height: origin.1 + height,
                items: vec![],
                paragraphs: vec![PageParagraphData {
                    block_index: 3,
                    layout: std::sync::Arc::new(para),
                    origin,
                }],
            },
            tile_width_pt: 436.0,
        }
    }

    #[test]
    fn reflow_hit_test_resolves_paragraph_and_offset() {
        let rl = one_para_reflow("Hello world", (5.0, 10.0));
        // Click inside the paragraph: returns its block_index and a byte offset.
        let (block, byte) = rl.reflow_hit_test(8.0, 12.0).expect("hit");
        assert_eq!(block, 3);
        assert!(byte <= "Hello world".len());
        // Far past the end of the line maps to the last offset.
        let (_, byte_end) = rl.reflow_hit_test(390.0, 12.0).expect("hit end");
        assert_eq!(byte_end, "Hello world".len());
        // Paginated layouts have no reflow hit-testing.
        assert_eq!(reflow(100.0, 400.0).reflow_hit_test(8.0, 12.0), None);
    }

    #[test]
    fn reflow_caret_is_offset_by_paragraph_origin() {
        let rl = one_para_reflow("Hello world", (5.0, 40.0));
        let cr = rl.reflow_cursor_canvas(3, 0).expect("caret at start");
        // Caret at byte 0 sits at the paragraph's canvas origin (x≈5, y≈40).
        assert!((cr.x - 5.0).abs() < 2.0, "x={}", cr.x);
        assert!((cr.y - 40.0).abs() < 4.0, "y={}", cr.y);
        assert!(cr.height > 0.0);
        // Unknown paragraph → None.
        assert!(rl.reflow_cursor_canvas(99, 0).is_none());
    }

    #[test]
    fn render_mode_width_tolerant_equality() {
        let a = RenderMode::Reflow {
            available_width_pt: 600.0,
        };
        let b = RenderMode::Reflow {
            available_width_pt: 600.3,
        };
        let c = RenderMode::Reflow {
            available_width_pt: 620.0,
        };
        assert!(a.matches(&b));
        assert!(!a.matches(&c));
        assert!(!a.matches(&RenderMode::Paginated));
    }
}
