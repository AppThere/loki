// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The residency **model**: which page tiles mount, and how many texture bytes
//! that costs.
//!
//! Pure arithmetic over page sizes and viewport geometry — no wgpu, no Dioxus,
//! no document. That is what lets Phase 2's budget be swept headlessly across
//! zoom × DPI (Spec 08 T2.5a, R16) instead of waiting on hardware.

/// Bytes per texel of a page texture.
///
/// The page format is `Rgba8Unorm` at every allocation site
/// (`loki-renderer/src/page_paint_render.rs`), so this is 4 and not a guess. A
/// format change must change this constant, or every byte figure in Phase 2
/// silently becomes wrong.
pub const BYTES_PER_TEXEL: u64 = 4;

/// CSS pixels per typographic point — the fixed 96/72 the tile layout applies
/// when turning a page's point size into its on-screen box.
pub const PT_TO_CSS_PX: f64 = 96.0 / 72.0;

/// Lowest and highest zoom the document view will accept.
///
/// # Why the residency model owns these rather than the renderer
///
/// They were a literal `zoom.clamp(0.25, 4.0)` in
/// `loki-renderer/src/doc_page_source_scale.rs`, which is the right place to
/// *apply* them and the wrong place to *define* them. Texture demand goes as the
/// square of zoom, so this pair is what bounds the whole residency problem: it
/// decides the peak byte figure any device can be asked for, and therefore
/// whether the survival regime (`plan::plan_residency` step 5) is reachable at
/// all.
/// `plan_reachability_tests::the_survival_regime_is_reachable_within_the_zoom_clamp`
/// depends on the value, so raising the clamp here re-runs that question instead
/// of silently changing the answer in another crate.
pub const MIN_ZOOM: f64 = 0.25;

/// See [`MIN_ZOOM`].
pub const MAX_ZOOM: f64 = 4.0;

// An inverted or empty range would make every zoom sweep in this crate — and the
// reachability answer that depends on one — silently vacuous. Checked at compile
// time rather than in a test, because a test asserting a relation between two
// constants is a lint (`clippy::assertions_on_constants`) and, more to the point,
// is checking something the compiler can simply refuse to build.
const _: () = assert!(MIN_ZOOM > 0.0 && MIN_ZOOM < MAX_ZOOM);

/// A page's paper size in points, the unit `loki-layout` reports.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PageBox {
    /// Page width in points.
    pub width_pt: f64,
    /// Page height in points.
    pub height_pt: f64,
}

impl PageBox {
    /// A page of `width_pt` × `height_pt`.
    #[must_use]
    pub fn new(width_pt: f64, height_pt: f64) -> Self {
        Self {
            width_pt,
            height_pt,
        }
    }

    /// ISO A4 — the default page of a new document
    /// (`loki_doc_model::layout::page::PageSize::a4`).
    #[must_use]
    pub fn a4() -> Self {
        Self::new(595.28, 841.89)
    }

    /// US Letter — the page S0.2's texture arithmetic is quoted against, kept
    /// so Phase 2's baseline is directly comparable with the spike's table.
    #[must_use]
    pub fn us_letter() -> Self {
        Self::new(612.0, 792.0)
    }

    /// The tile's on-screen box in CSS px at `zoom`.
    ///
    /// Mirrors `document_view.rs`, which sizes each tile `pt × 96/72 × zoom`.
    #[must_use]
    pub fn css_size(self, zoom: f64) -> (f64, f64) {
        (
            self.width_pt * PT_TO_CSS_PX * zoom,
            self.height_pt * PT_TO_CSS_PX * zoom,
        )
    }

    /// The physical texture dimensions in device pixels.
    ///
    /// The paint source is handed already-physical dimensions by Blitz and
    /// clamps each to at least 1 (`page_paint_source.rs`), so the model applies
    /// the device scale factor to the CSS box and clamps the same way.
    ///
    /// **Not established:** that Blitz's own layout rounding produces exactly
    /// `round(css × dsf)`. A ±1 px disagreement on either axis is under 0.1% of
    /// a page-sized texture, but it is an assumption of the model and not an
    /// observation of the tree — the counter, read on device, is what would
    /// settle it.
    #[must_use]
    pub fn texture_size_px(self, zoom: f64, device_scale_factor: f64) -> (u32, u32) {
        let (w_css, h_css) = self.css_size(zoom);
        (
            to_device_px(w_css * device_scale_factor),
            to_device_px(h_css * device_scale_factor),
        )
    }

    /// Requested texture bytes for this page at `zoom` and `device_scale_factor`.
    #[must_use]
    pub fn texture_bytes(self, zoom: f64, device_scale_factor: f64) -> u64 {
        let (w, h) = self.texture_size_px(zoom, device_scale_factor);
        texture_bytes(w, h)
    }
}

/// Rounds a CSS-derived length to whole device pixels, clamped to `[1, u32::MAX]`.
///
/// A non-finite input is a bug upstream, not something to propagate as a
/// gigantic allocation, so it collapses to the same 1 px floor the paint source
/// applies.
fn to_device_px(v: f64) -> u32 {
    if !v.is_finite() || v <= 1.0 {
        return 1;
    }
    let rounded = v.round();
    if rounded >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        rounded as u32
    }
}

/// Requested bytes for one `Rgba8Unorm` texture of `width_px` × `height_px`.
#[must_use]
pub fn texture_bytes(width_px: u32, height_px: u32) -> u64 {
    u64::from(width_px) * u64::from(height_px) * BYTES_PER_TEXEL
}

/// The viewport state that decides which tiles mount and how large they are.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ViewportSpec {
    /// Scroll offset of the document container, in CSS px.
    pub scroll_top_px: f64,
    /// Visible height of the container, in CSS px.
    pub client_height_px: f64,
    /// Vertical gap between page tiles, in CSS px
    /// (`appthere_ui::tokens::layout::PAGE_GAP_PX` = 24 in the editor).
    pub page_gap_px: f64,
    /// User zoom factor — 1.0 is 100%.
    pub zoom: f64,
    /// Display device scale factor — 1.0 is a standard-DPI display, 2.0 HiDPI.
    pub device_scale_factor: f64,
}

impl ViewportSpec {
    /// A viewport of `client_height_px` scrolled to `scroll_top_px`, at the
    /// editor's 24 px page gap.
    #[must_use]
    pub fn new(
        scroll_top_px: f64,
        client_height_px: f64,
        zoom: f64,
        device_scale_factor: f64,
    ) -> Self {
        Self {
            scroll_top_px,
            client_height_px,
            page_gap_px: 24.0,
            zoom,
            device_scale_factor,
        }
    }
}

/// Returns, per page in document order, whether it falls within the tile
/// virtualization window: the visible range `[viewport_top, viewport_top + vh]`
/// grown by one screen (`vh`) on each side.
///
/// `page_heights` are CSS-px heights in document order, separated by `gap_px`.
/// A document shorter than the window has every page visible, so short
/// documents transparently render everything.
///
/// This is the production mounting rule — `loki_renderer::virtualize` calls it
/// — not a restatement of it. Phase 2's model and Phase 2's behaviour cannot
/// disagree because there is only one of them.
#[must_use]
pub fn visible_window(
    page_heights: &[f64],
    gap_px: f64,
    viewport_top: f64,
    viewport_height: f64,
) -> Vec<bool> {
    let vh = viewport_height.max(1.0);
    let win_lo = viewport_top - vh;
    let win_hi = viewport_top + 2.0 * vh;
    let mut page_top = 0.0_f64;
    page_heights
        .iter()
        .map(|&h| {
            // Overlap test between [page_top, page_top + h] and [win_lo, win_hi].
            let visible = (page_top + h) >= win_lo && page_top <= win_hi;
            page_top += h + gap_px;
            visible
        })
        .collect()
}

/// Which pages of `pages` hold a GPU tile under `vp`.
#[must_use]
pub fn resident_pages(pages: &[PageBox], vp: &ViewportSpec) -> Vec<bool> {
    let heights: Vec<f64> = pages.iter().map(|p| p.css_size(vp.zoom).1).collect();
    visible_window(
        &heights,
        vp.page_gap_px,
        vp.scroll_top_px,
        vp.client_height_px,
    )
}

/// Total requested texture bytes resident under `vp`.
///
/// This is Phase 2's governing metric. Note what it is *not* a function of:
/// `pages.len()` enters only through which pages the window happens to cover,
/// so a 500-page and a 10-page document at the same zoom report the same figure
/// once both exceed the window (S0.2 §3; asserted in the Phase 2 bench).
#[must_use]
pub fn resident_texture_bytes(pages: &[PageBox], vp: &ViewportSpec) -> u64 {
    pages
        .iter()
        .zip(resident_pages(pages, vp))
        .filter(|(_, visible)| *visible)
        .map(|(page, _)| page.texture_bytes(vp.zoom, vp.device_scale_factor))
        .sum()
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
