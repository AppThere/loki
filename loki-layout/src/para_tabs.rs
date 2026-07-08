// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tab-stop resolution, leader fills, and per-tab expansion planning (gap #7).
//! Split out of `para.rs` (Phase 7.1). `layout_paragraph_uncached` (in
//! `para.rs`) calls `compute_tab_plans` / `emit_tab_leader` and uses `TabPlan`.

use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader};

use crate::color::LayoutColor;
use crate::geometry::LayoutRect;
use crate::items::{PositionedItem, PositionedRect};

use super::ResolvedTabStop;

// ── Tab stop helpers (gap #7) ─────────────────────────────────────────────────

// TODO(tab-default): use Document.settings.default_tab_stop_pt once
// DocumentSettings is threaded through layout_document.
/// Default tab stop interval: 0.5 inch = 36 pt = 720 twips (Word default).
const DEFAULT_TAB_INTERVAL: f32 = 36.0;

/// Return the tab stop a tab at pen position `x` advances to: the first
/// explicit stop strictly greater than `x`, else a synthesized default-grid
/// stop (36 pt, left-aligned, no leader). A hanging indent acts as an implicit
/// first stop.
fn next_tab_stop_resolved(
    stops: &[ResolvedTabStop],
    x: f32,
    indent_hanging: f32,
) -> ResolvedTabStop {
    if indent_hanging > 0.0 && x < indent_hanging - 0.5 {
        return ResolvedTabStop {
            position: indent_hanging,
            alignment: TabAlignment::Left,
            leader: TabLeader::None,
        };
    }
    if let Some(s) = stops.iter().find(|s| s.position > x + 0.5) {
        *s
    } else {
        ResolvedTabStop {
            position: ((x / DEFAULT_TAB_INTERVAL).floor() + 1.0) * DEFAULT_TAB_INTERVAL,
            alignment: TabAlignment::Left,
            leader: TabLeader::None,
        }
    }
}

/// Emit the leader fill for a tab gap `[x0, x1]` at `baseline`, as
/// renderer-agnostic [`PositionedItem::FilledRect`]s. Dotted leaders are a row
/// of small squares; dashed are short bars; underscore/heavy are a solid rule
/// just below the baseline (like an underline). A `None` leader emits nothing.
pub(super) fn emit_tab_leader(
    items: &mut Vec<PositionedItem>,
    leader: TabLeader,
    x0: f32,
    x1: f32,
    baseline: f32,
) {
    let width = x1 - x0;
    if width < 1.0 || leader == TabLeader::None {
        return;
    }
    let color = LayoutColor::BLACK;
    let mut dots = |size: f32, pitch: f32, y: f32| {
        let mut x = x0 + (pitch - size) * 0.5;
        while x + size <= x1 {
            items.push(PositionedItem::FilledRect(PositionedRect {
                rect: LayoutRect::new(x, y, size, size),
                color,
            }));
            x += pitch;
        }
    };
    match leader {
        TabLeader::Dot | TabLeader::MiddleDot => dots(0.9, 3.6, baseline - 1.6),
        TabLeader::Dash => {
            let (dash, pitch, th, y) = (2.4, 4.2, 0.8, baseline - 1.9);
            let mut x = x0 + 1.0;
            while x + dash <= x1 {
                items.push(PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(x, y, dash, th),
                    color,
                }));
                x += pitch;
            }
        }
        TabLeader::Underscore => items.push(PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(x0, baseline + 1.0, width, 0.8),
            color,
        })),
        TabLeader::Heavy => items.push(PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(x0, baseline + 1.0, width, 1.4),
            color,
        })),
        // `None` is handled by the early return; `_` covers the non-exhaustive
        // enum's future variants.
        _ => {}
    }
}

/// The planned expansion of one tab character: the inline-box width to insert
/// so the following text lands at its stop, plus the leader to draw across it.
#[derive(Debug, Clone, Copy)]
pub(super) struct TabPlan {
    /// Width of the inline box that advances the pen to the aligned position.
    pub(super) width: f32,
    /// Leader to fill the gap (drawn across `[tab_x, tab_x + width]`).
    pub(super) leader: TabLeader,
}

/// Compute each tab's expansion width and leader from probe measurements.
///
/// Processes tabs left-to-right, accumulating the shift each expansion adds, so
/// a later tab's stop is found relative to its *shifted* position. Alignment
/// positions the content that follows the tab (up to the next tab / line end):
/// Left advances to the stop; Right ends the content at the stop; Center
/// centres it; Decimal places the first `.` at the stop. Content widths come
/// from the zero-width probe boxes (natural, unshifted layout).
#[allow(clippy::too_many_arguments)]
pub(super) fn compute_tab_plans(
    stops: &[ResolvedTabStop],
    indent_hanging: f32,
    x_tab: &[f32],
    line_tab: &[usize],
    x_dec: &[f32],
    x_end: f32,
    line_end: usize,
) -> Vec<TabPlan> {
    let n = x_tab.len();
    let mut plans = Vec::with_capacity(n);
    let mut shift = 0.0f32;
    for i in 0..n {
        let final_tab_x = x_tab[i] + shift;
        let stop = next_tab_stop_resolved(stops, final_tab_x, indent_hanging);

        // Natural boundary of the content following this tab: the next tab, or
        // the end-of-text sentinel for the last tab.
        let (boundary_x, boundary_line) = if i + 1 < n {
            (x_tab[i + 1], line_tab[i + 1])
        } else {
            (x_end, line_end)
        };
        // Content width is only meaningful when the boundary is on the same
        // visual line; otherwise the content wrapped — fall back to left-align.
        let content_w = if boundary_line == line_tab[i] && boundary_x >= x_tab[i] {
            boundary_x - x_tab[i]
        } else {
            0.0
        };

        let offset = match stop.alignment {
            TabAlignment::Right => content_w,
            TabAlignment::Center => content_w / 2.0,
            TabAlignment::Decimal => {
                if x_dec[i].is_nan() {
                    content_w // no decimal separator → behave like right-align
                } else {
                    (x_dec[i] - x_tab[i]).max(0.0)
                }
            }
            // Left / Clear (filtered earlier) / non-exhaustive → advance to stop.
            _ => 0.0,
        };

        let width = (stop.position - offset - final_tab_x).max(0.0);
        plans.push(TabPlan {
            width,
            leader: stop.leader,
        });
        shift += width;
    }
    plans
}
