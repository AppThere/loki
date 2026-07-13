// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The reflow view's width and type-scale metrics — the **single source**
//! (Spec 01 A-1) for the reading measure, the responsive type scale
//! (Spec 03 M4 / D5), and the px↔pt conversions that paint, hit-testing,
//! and keyboard navigation must all agree on. Split out of
//! `render_layout.rs` (file ceiling); re-exported from there, so callers
//! keep the `render_layout::…` paths.

/// Horizontal reading-margin inset in points (24 CSS px each side): reflow is
/// computed at `tile width − 2 × inset` and painted shifted right by it.
pub const REFLOW_PADDING_PT: f32 = 18.0;

/// Narrowest reflow content width the layout engine is asked for, in points.
/// Guards against degenerate layouts while the viewport width is still being
/// measured.
pub const MIN_REFLOW_CONTENT_PT: f32 = 50.0;

/// Widest reflow **tile** (CSS px) — caps the reading measure so the
/// non-paginated view holds a comfortable line length and **centres** on wide
/// windows (Spec 03 M4 / D5); below this the tile tracks the viewport. Matches
/// the HTML reflow fallback's `max-width` so both paths read identically.
pub const MAX_REFLOW_TILE_PX: f32 = 820.0;

/// CSS pixels → layout points (72 dpi / 96 dpi).
pub const PX_TO_PT: f32 = 72.0 / 96.0;

/// The reflow **tile** width (CSS px) for a measured viewport: the viewport
/// width capped at [`MAX_REFLOW_TILE_PX`] (the renderer centres the tile, so
/// the cap centres the reading column). The single source of reflow width for
/// paint, hit-testing, and keyboard navigation (Spec 01 A-1).
#[must_use]
pub fn reflow_tile_width_px(viewport_width_px: f32) -> f32 {
    viewport_width_px.clamp(0.0, MAX_REFLOW_TILE_PX)
}

/// Compact-breakpoint threshold in CSS px. Must equal
/// `appthere_ui::tokens::layout::BREAKPOINT_COMPACT_MAX_PX` — loki-renderer
/// deliberately does not depend on the UI crate, so the value is duplicated
/// and drift-locked by `type_scale_threshold_matches_the_breakpoint`
/// (`loki-text`, which sees both crates).
pub const REFLOW_COMPACT_MAX_PX: f32 = 600.0;

/// Responsive reflow type scale for Compact viewports (Spec 03 M4 / D5):
/// phone-width reading renders the type 12.5 % larger so Compact is not just
/// a shrunk Expanded. Strictly a **view transform** — the document's own
/// point sizes are untouched (rescaling them is a fidelity concern, per the
/// Spec 03 audit) — implemented by laying out at `width ÷ scale` and painting
/// at `zoom = scale`, so the on-screen tile width is unchanged.
pub const REFLOW_COMPACT_TYPE_SCALE: f32 = 1.125;

/// The reflow type scale for a measured viewport width: Compact widths get
/// [`REFLOW_COMPACT_TYPE_SCALE`], everything else 1.0. An unmeasured width
/// (`<= 1 px`) stays 1.0 so the first frame does not flash scaled type.
#[must_use]
pub fn reflow_type_scale(viewport_width_px: f32) -> f32 {
    if viewport_width_px > 1.0 && viewport_width_px < REFLOW_COMPACT_MAX_PX {
        REFLOW_COMPACT_TYPE_SCALE
    } else {
        1.0
    }
}

/// The reflow tile width the **layout engine** fills, in points: the CSS tile
/// width converted to points and divided by [`reflow_type_scale`]. Painting
/// at the scale restores the on-screen tile width with visually larger type.
/// The single source of reflow layout width for paint, hit-testing, and
/// keyboard navigation (Spec 01 A-1).
#[must_use]
pub fn reflow_layout_tile_width_pt(viewport_width_px: f32) -> f32 {
    reflow_tile_width_px(viewport_width_px) * PX_TO_PT / reflow_type_scale(viewport_width_px)
}

/// The reflow **content** width the layout engine fills (the reading measure,
/// in points): the layout tile minus the [`REFLOW_PADDING_PT`] side insets,
/// floored at [`MIN_REFLOW_CONTENT_PT`]. Hit-testing and navigation must
/// build their layout at exactly this width to match the paint.
#[must_use]
pub fn reflow_layout_content_width_pt(viewport_width_px: f32) -> f32 {
    (reflow_layout_tile_width_pt(viewport_width_px) - 2.0 * REFLOW_PADDING_PT)
        .max(MIN_REFLOW_CONTENT_PT)
}
