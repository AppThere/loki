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
use dioxus::html::geometry::PixelsVector2D;
use dioxus::prelude::*;

/// Captured `MountedData` for the scroll container, used to drive programmatic
/// scrolling when the user drags a thumb.
pub(super) type CanvasMounted = Signal<Option<MountedEvent>>;

/// In-progress thumb drag: `(pointer_start, scroll_start)` in logical pixels.
pub(super) type ThumbDrag = Signal<Option<(f32, f32)>>;

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

/// Vertical scroll indicator + drag handle — a right-edge gutter (always present
/// so the layout is stable). The thumb reflects position and can be dragged with
/// the mouse to scroll; touch users scroll the page directly. `current_page` /
/// `total_pages` provide the pre-measurement fallback.
pub(super) fn vertical_scrollbar(
    metrics: ScrollMetrics,
    current_page: u32,
    total_pages: u32,
    mounted: CanvasMounted,
    mut drag: ThumbDrag,
) -> Element {
    let (thumb_frac, start_frac) = thumb_geometry(
        metrics.client_height,
        metrics.scroll_height,
        metrics.scroll_top,
        Some((current_page, total_pages)),
    );
    let on_move = move |y: f32| scroll_axis(mounted, drag, true, y, metrics);
    rsx! {
        div {
            // A spacer + thumb in a flex column: percentage heights resolve
            // against the track's (definite) height, avoiding the CSS rule that
            // makes percentage *margins* resolve against width. Drag handlers on
            // the track keep tracking the pointer as it moves over the gutter.
            style: format!(
                "flex-shrink: 0; width: {w}px; box-sizing: border-box; \
                 padding: 2px; background: {track}; display: flex; \
                 flex-direction: column;",
                w = TRACK_PX,
                track = tokens::COLOR_SCROLLBAR_TRACK,
            ),
            onmousemove: move |e: MouseEvent| on_move(e.client_coordinates().y as f32),
            onmouseup: move |_| drag.set(None),
            onmouseleave: move |_| drag.set(None),
            div { style: format!("flex-shrink: 0; height: {}%;", start_frac * 100.0) }
            div {
                style: format!(
                    "flex-shrink: 0; width: 100%; height: {h}%; \
                     background: {thumb}; border-radius: {r}px; cursor: grab;",
                    h = thumb_frac * 100.0,
                    thumb = tokens::COLOR_SCROLLBAR_THUMB,
                    r = (TRACK_PX - 4.0) / 2.0,
                ),
                onmousedown: move |e: MouseEvent| {
                    drag.set(Some((e.client_coordinates().y as f32, metrics.scroll_top)));
                },
            }
        }
    }
}

/// Horizontal scroll indicator + drag handle — rendered along the bottom **only**
/// when the content is wider than the viewport (a wide page on a narrow screen,
/// or while zoomed in). Empty fragment otherwise so no vertical space is wasted.
pub(super) fn horizontal_scrollbar(
    metrics: ScrollMetrics,
    mounted: CanvasMounted,
    mut drag: ThumbDrag,
) -> Element {
    if !metrics.can_scroll_x() {
        return rsx! {};
    }
    let (thumb_frac, start_frac) = thumb_geometry(
        metrics.client_width,
        metrics.scroll_width,
        metrics.scroll_left,
        None,
    );
    let on_move = move |x: f32| scroll_axis(mounted, drag, false, x, metrics);
    rsx! {
        div {
            style: format!(
                "flex-shrink: 0; height: {h}px; box-sizing: border-box; \
                 padding: 2px; background: {track}; display: flex; \
                 flex-direction: row;",
                h = TRACK_PX,
                track = tokens::COLOR_SCROLLBAR_TRACK,
            ),
            onmousemove: move |e: MouseEvent| on_move(e.client_coordinates().x as f32),
            onmouseup: move |_| drag.set(None),
            onmouseleave: move |_| drag.set(None),
            div { style: format!("flex-shrink: 0; width: {}%;", start_frac * 100.0) }
            div {
                style: format!(
                    "flex-shrink: 0; height: 100%; width: {w}%; \
                     background: {thumb}; border-radius: {r}px; cursor: grab;",
                    w = thumb_frac * 100.0,
                    thumb = tokens::COLOR_SCROLLBAR_THUMB,
                    r = (TRACK_PX - 4.0) / 2.0,
                ),
                onmousedown: move |e: MouseEvent| {
                    drag.set(Some((e.client_coordinates().x as f32, metrics.scroll_left)));
                },
            }
        }
    }
}

/// Map a thumb drag along one axis to a `MountedData::scroll` call. `pointer` is
/// the current pointer position on the drag axis (logical px). No-op until the
/// canvas has mounted, a drag is active, and the axis is scrollable.
fn scroll_axis(
    mounted: CanvasMounted,
    drag: ThumbDrag,
    vertical: bool,
    pointer: f32,
    metrics: ScrollMetrics,
) {
    let Some((start_pointer, start_scroll)) = drag() else {
        return;
    };
    let (client, max_scroll) = if vertical {
        (metrics.client_height, metrics.scroll_height)
    } else {
        (metrics.client_width, metrics.scroll_width)
    };
    if client <= 0.0 || max_scroll <= 0.0 {
        return;
    }
    // Track length ≈ the client extent; thumb→scroll gain is content/track.
    let ratio = (client + max_scroll) / client;
    let target = (start_scroll + (pointer - start_pointer) * ratio).clamp(0.0, max_scroll);
    let guard = mounted();
    let Some(m) = guard.as_ref() else { return };
    let coords = if vertical {
        PixelsVector2D::new(metrics.scroll_left as f64, target as f64)
    } else {
        PixelsVector2D::new(target as f64, metrics.scroll_top as f64)
    };
    // The patched backing performs the scroll eagerly (posts the event before
    // returning a ready future), so the returned future is safe to drop here in
    // this synchronous handler.
    drop(m.scroll(coords, ScrollBehavior::Instant));
}
