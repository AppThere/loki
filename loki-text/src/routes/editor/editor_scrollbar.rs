// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Custom scroll-position indicators for the document canvas.
//!
//! Blitz (blitz-paint 0.2.x) paints **no** scrollbar chrome and Stylo's
//! `scrollbar-*` properties are inert, so the editor draws its own thin track +
//! thumb.  The thumb reflects the current scroll position and the visible
//! fraction of the content; it is **not draggable** — dioxus-native 0.7 exposes
//! no programmatic scroll API (`convert_mounted_data` / `scroll_to` are
//! `unimplemented!()`), so the content is moved by touch-drag / wheel and the
//! indicator follows.  Scrolling itself is driven by the patched Blitz shell.
//!
//! ## Geometry
//!
//! The DOM `scroll` event (PATCH(loki) in dioxus-native-dom) reports Taffy
//! geometry where `scroll_width` / `scroll_height` are the **scrollable
//! distance** (content − client), not the total content size.  Total content
//! size is therefore `client + max_scroll`, and the thumb covers
//! `client / (client + max_scroll)` of the track.

use appthere_ui::tokens;
use dioxus::prelude::*;

/// Track / gutter thickness in logical pixels (reserved on the right and,
/// when content overflows horizontally, along the bottom).
const TRACK_PX: f32 = 12.0;

/// Smallest thumb length as a fraction of the track, so a long document still
/// shows a grabbable-looking indicator instead of a sliver.
const MIN_THUMB_FRAC: f32 = 0.08;

/// Live scroll geometry for the canvas container, mirrored from the most recent
/// DOM `scroll` event.  All values are logical pixels; `scroll_width` /
/// `scroll_height` are the scrollable distance (see module docs).  Defaults to
/// all-zero (pre-first-scroll), which callers treat as "not yet measured".
#[derive(Clone, Copy, PartialEq, Default)]
pub(super) struct ScrollMetrics {
    pub scroll_top: f32,
    pub scroll_left: f32,
    pub scroll_width: f32,
    pub scroll_height: f32,
    pub client_width: f32,
    pub client_height: f32,
}

impl ScrollMetrics {
    /// True when the content can be scrolled horizontally — the only case in
    /// which the bottom scrollbar is shown.
    fn can_scroll_x(&self) -> bool {
        self.client_width > 0.0 && self.scroll_width > 0.5
    }
}

/// Returns `(thumb_fraction, start_fraction)` of the track for one axis.
///
/// `max_scroll` is the scrollable distance and `offset` the current scroll
/// position.  When geometry has not been measured yet (`client`/`max_scroll`
/// still zero) but the document spans multiple pages, falls back to an
/// approximate position derived from page progress so the bar is visible from
/// load; otherwise returns a full-length thumb (content fits — nothing to do).
fn thumb_geometry(
    client: f32,
    max_scroll: f32,
    offset: f32,
    page_fallback: Option<(u32, u32)>,
) -> (f32, f32) {
    if client > 0.0 && max_scroll > 0.5 {
        let content = client + max_scroll;
        let thumb = (client / content).clamp(MIN_THUMB_FRAC, 1.0);
        let start = (offset / content).clamp(0.0, 1.0 - thumb);
        return (thumb, start);
    }
    if let Some((current, total)) = page_fallback
        && total > 1
    {
        let total = total as f32;
        let thumb = (1.0 / total).clamp(MIN_THUMB_FRAC, 1.0);
        let start = ((current.max(1) - 1) as f32 / total).clamp(0.0, 1.0 - thumb);
        return (thumb, start);
    }
    (1.0, 0.0)
}

/// Vertical scroll indicator — always rendered as a right-edge gutter so the
/// layout is stable; the thumb reflects vertical position.  `current_page` /
/// `total_pages` provide the pre-measurement fallback.
pub(super) fn vertical_scrollbar(
    metrics: ScrollMetrics,
    current_page: u32,
    total_pages: u32,
) -> Element {
    let (thumb_frac, start_frac) = thumb_geometry(
        metrics.client_height,
        metrics.scroll_height,
        metrics.scroll_top,
        Some((current_page, total_pages)),
    );
    rsx! {
        div {
            // A spacer + thumb in a flex column: percentage heights resolve
            // against the track's (definite) height, avoiding the CSS rule that
            // makes percentage *margins* resolve against width.
            style: format!(
                "flex-shrink: 0; width: {w}px; box-sizing: border-box; \
                 padding: 2px; background: {track}; display: flex; \
                 flex-direction: column;",
                w = TRACK_PX,
                track = tokens::COLOR_SCROLLBAR_TRACK,
            ),
            div { style: format!("flex-shrink: 0; height: {}%;", start_frac * 100.0) }
            div {
                style: format!(
                    "flex-shrink: 0; width: 100%; height: {h}%; \
                     background: {thumb}; border-radius: {r}px;",
                    h = thumb_frac * 100.0,
                    thumb = tokens::COLOR_SCROLLBAR_THUMB,
                    r = (TRACK_PX - 4.0) / 2.0,
                ),
            }
        }
    }
}

/// Horizontal scroll indicator — rendered along the bottom **only** when the
/// content is wider than the viewport (e.g. a wide page on a narrow phone, or
/// while zoomed in).  Returns an empty fragment otherwise so no vertical space
/// is wasted when there is nothing to scroll horizontally.
pub(super) fn horizontal_scrollbar(metrics: ScrollMetrics) -> Element {
    if !metrics.can_scroll_x() {
        return rsx! {};
    }
    let (thumb_frac, start_frac) = thumb_geometry(
        metrics.client_width,
        metrics.scroll_width,
        metrics.scroll_left,
        None,
    );
    rsx! {
        div {
            style: format!(
                "flex-shrink: 0; height: {h}px; box-sizing: border-box; \
                 padding: 2px; background: {track}; display: flex; \
                 flex-direction: row;",
                h = TRACK_PX,
                track = tokens::COLOR_SCROLLBAR_TRACK,
            ),
            div { style: format!("flex-shrink: 0; width: {}%;", start_frac * 100.0) }
            div {
                style: format!(
                    "flex-shrink: 0; height: 100%; width: {w}%; \
                     background: {thumb}; border-radius: {r}px;",
                    w = thumb_frac * 100.0,
                    thumb = tokens::COLOR_SCROLLBAR_THUMB,
                    r = (TRACK_PX - 4.0) / 2.0,
                ),
            }
        }
    }
}
