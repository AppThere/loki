// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Keeping a point still while the zoom changes (Spec 08 T5.6).
//!
//! # Why this is a *trigger*, and never a subscription
//!
//! Phase 1's I-20 fix made the caret reveal fire on caret **movement** and
//! nothing else, because keying it on anything derived from the scroll position
//! makes the wheel fight the reveal: the user scrolls, the caret's
//! viewport-relative position changes without the caret moving, a band check
//! fails, and the view is dragged back.
//!
//! Anchoring is the same hazard wearing the opposite coat. The tempting
//! implementation is to widen the caret-follow effect so it also watches zoom —
//! and that reintroduces I-20 exactly, because the effect would then re-run on a
//! scroll-derived quantity. So anchoring is computed **at the moment the zoom
//! changes**, by the code that changed it, and applied with one `scroll_to`. No
//! effect subscribes to anything new.
//!
//! # The arithmetic
//!
//! Scaled content does not start at scroll offset zero. A scroll container
//! usually has padding, and that padding is CSS — it does **not** scale with the
//! zoom. So a content point `d` sits at viewport offset `a` when
//!
//! ```text
//! a = p + d·z − s
//! ```
//!
//! where `p` is the unscaled offset from the scrollport's origin to the scaled
//! content's. Holding `a` fixed across a zoom change gives
//!
//! ```text
//! s₁ = (s₀ + a − p)·(z₁/z₀) + p − a
//! ```
//!
//! **`p` is not a refinement, it is the model.** Dropping it (the first draft
//! did) leaves a drift of `p·(1 − z₁/z₀)`, which is 5 px for a wheel notch and
//! 120 px across the control's full range with this app's 24 px padding — small
//! enough at one step to read as imprecision and large enough over a gesture to
//! read as the anchor not working. A screen sitting measured −5 px where the
//! model predicted −5.04.
//!
//! # The one other thing that makes it wrong
//!
//! Using the **effective** zoom on one side and the **requested** zoom on the
//! other. Spec 08 T5.4 stores those separately and they differ whenever the
//! capability cap bites, so a caller that mixes them anchors against a ratio no
//! rendering ever used. Both arguments here are the zoom the *content was and
//! will be painted at* — see [`anchored_scroll`]'s note.

/// The point held still across a zoom change.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ZoomAnchor {
    /// A position in the viewport, in CSS pixels from its top-left corner.
    ///
    /// Ctrl+scroll and pinch use this: the content under the pointer is what the
    /// reader is looking at, and it is where they expect the zoom to happen.
    Viewport {
        /// Pixels from the viewport's left edge.
        x: f32,
        /// Pixels from the viewport's top edge.
        y: f32,
    },
    /// The centre of the viewport.
    ///
    /// What a button, menu or preset uses when there is no caret to anchor to.
    /// The reader pressing a zoom control in the status bar is looking at the
    /// page, not at the control.
    Centre,
}

impl ZoomAnchor {
    /// The anchor as a viewport offset, given the viewport's size.
    #[must_use]
    pub fn offset(self, client_width: f32, client_height: f32) -> (f32, f32) {
        match self {
            Self::Viewport { x, y } => (x, y),
            Self::Centre => (client_width / 2.0, client_height / 2.0),
        }
    }
}

/// The scroll offset that leaves the anchored content point where it was.
///
/// `from_zoom` and `to_zoom` must both be the zoom the content is **painted**
/// at — the effective zoom, not the requested one. They are the same number
/// whenever no capability cap is in force, which is what makes mixing them a
/// mistake that hides on most machines and appears on the small ones.
///
/// Returns the offsets clamped at zero. The *upper* bound is deliberately not
/// applied: the new scroll extent is not known until the content has been laid
/// out at the new zoom, and a clamp against the old extent would under-scroll
/// exactly when zooming in — which is the common direction. The scroll container
/// clamps for us, and clamping twice against different extents is how a
/// scroll-to lands short.
///
/// `content_origin` is the **unscaled** offset from the scrollport's top-left to
/// the scaled content's — a scroll container's own padding, typically. It is
/// separate from `scroll` because it does not move and does not scale; see the
/// module's arithmetic for why omitting it is a drift and not a rounding error.
#[must_use]
pub fn anchored_scroll(
    scroll: (f32, f32),
    viewport: (f32, f32),
    content_origin: (f32, f32),
    anchor: ZoomAnchor,
    from_zoom: f32,
    to_zoom: f32,
) -> (f32, f32) {
    if !(from_zoom.is_finite() && to_zoom.is_finite()) || from_zoom <= 0.0 || to_zoom <= 0.0 {
        return scroll;
    }
    let (ax, ay) = anchor.offset(viewport.0, viewport.1);
    let (px, py) = content_origin;
    let ratio = to_zoom / from_zoom;
    let x = (scroll.0 + ax - px).mul_add(ratio, px - ax).max(0.0);
    let y = (scroll.1 + ay - py).mul_add(ratio, py - ay).max(0.0);
    (x, y)
}

#[cfg(test)]
#[path = "zoom_anchor_tests.rs"]
mod tests;
