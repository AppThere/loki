// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`ViewportController`] — the one handle on a scroll container (Spec 08 T1.2).

use dioxus::html::geometry::PixelsVector2D;
use dioxus::prelude::*;

use super::animate::MotionPreference;
use super::controller_offset::{effective_offset, Commanded};
use super::metrics::ScrollMetrics;
use super::reveal::{reveal_offset, RevealMargin};
use super::zoom_anchor::{anchored_scroll, ZoomAnchor};

/// A target rect in **content** coordinates: `(x, y, width, height)`.
pub type ContentRect = (f32, f32, f32, f32);

/// Owns everything a caller needs to observe and drive one scroll container.
///
/// Deliberately built **on** the existing `ScrollMetrics` signal rather than
/// beside it: the metrics a scrollbar already mirrors from `onscroll` are the
/// same numbers a reveal needs, and a second source would recreate the
/// divergence Spec 01 audit A-1 removed from viewport width (S0.1 §3).
///
/// # A command must never subscribe to scroll state (L08-019)
///
/// This is the rule I-20 was created by breaking. `scroll_to_reveal` read the
/// metrics signal reactively, so the caret-follow effect that called it became
/// a subscriber to every scroll event. Turning the wheel then re-ran the
/// effect, which recomputed the caret's position *relative to the new scroll
/// offset*, found it outside the reveal margin — because the user had just
/// scrolled it there — and scrolled back. The wheel was capped at the margin
/// band around the caret, and the cap was asymmetric (one line up, three down
/// at the margin values then in force) because the margin is.
///
/// Nothing in the type system stops that recurring, so the discipline is:
/// **observation methods read, command methods peek.** `metrics` and
/// `visible_rect` are the observers and say so; every command path goes
/// through [`Self::metrics_now`]. If a command ever needs a value not exposed
/// that way, add a peeking accessor rather than reaching for the reactive one.
///
/// The deeper guarantee lives at the call site: a reveal fires on *caret
/// revision change*, never on anything derived from scroll position. Removing
/// the subscription stops the loop; keying the trigger on caret identity means
/// a future subscription slipping back in cannot restart it.
///
/// `Copy`, so it threads through render functions like any other signal bundle.
#[derive(Clone, Copy)]
pub struct ViewportController {
    metrics: Signal<ScrollMetrics>,
    mounted: Signal<Option<MountedEvent>>,
    /// Bumped on every new scroll command. An in-flight animation compares this
    /// against the generation it started with and exits when superseded, so a
    /// second reveal replaces the first instead of fighting it (T1.1).
    generation: Signal<u64>,
    motion: Signal<MotionPreference>,
    /// The last offset commanded, with the metrics it was commanded against.
    ///
    /// See [`super::controller_offset`] — this exists because the metrics signal
    /// is a *report* from the DOM and necessarily lags the command that caused
    /// it.
    commanded: Signal<Option<Commanded>>,
}

/// Creates a [`ViewportController`] over an existing metrics signal and the
/// `MountedData` captured from the container's `onmounted`.
///
/// Call once per container, in a component.
pub fn use_viewport_controller(
    metrics: Signal<ScrollMetrics>,
    mounted: Signal<Option<MountedEvent>>,
) -> ViewportController {
    let generation = use_signal(|| 0_u64);
    let motion = use_signal(MotionPreference::default);
    let commanded = use_signal(|| None);
    ViewportController {
        metrics,
        mounted,
        generation,
        motion,
        commanded,
    }
}

impl ViewportController {
    /// The live scroll geometry, as a **reactive** read.
    ///
    /// Calling this inside a `use_effect` subscribes that effect to every
    /// scroll event. That is correct for an observer — a scroll indicator, a
    /// page counter — and catastrophic for anything that issues a scroll. See
    /// the type docs; commands use [`Self::metrics_now`].
    #[must_use]
    pub fn metrics(&self) -> ScrollMetrics {
        *self.metrics.read()
    }

    /// The visible region in content coordinates, `(x, y, width, height)`, as a
    /// **reactive** read. Same subscription caveat as [`Self::metrics`].
    #[must_use]
    pub fn visible_rect(&self) -> ContentRect {
        self.metrics.read().visible_rect()
    }

    /// The scroll offset a command should compose against, which is **not**
    /// always the one the metrics report — see [`effective_offset`].
    fn offset_now(&self) -> (f32, f32) {
        effective_offset(self.metrics_now(), *self.commanded.peek())
    }

    /// The animation epoch, for the tick loop in [`super::controller_animate`].
    pub(super) fn generation_now(&self) -> u64 {
        *self.generation.peek()
    }

    /// The current scroll geometry **without subscribing** — the only accessor
    /// a command path may use (L08-019).
    fn metrics_now(&self) -> ScrollMetrics {
        *self.metrics.peek()
    }

    /// Sets the motion preference (see [`MotionPreference`]).
    pub fn set_motion(&mut self, preference: MotionPreference) {
        self.motion.set(preference);
    }

    /// Cancels any in-flight smooth scroll, leaving the offset where it is.
    ///
    /// Called when the user takes the wheel — literally. An animation that
    /// keeps running through a user gesture is the "fighting the user"
    /// failure T1.4 exists to prevent.
    pub fn cancel(&mut self) {
        *self.generation.write() += 1;
    }

    /// Scrolls so `rect` is visible with `margin` of clear space around it.
    ///
    /// A no-op when the rect is already visible with its margins intact, when
    /// the container has not been measured, or when the container cannot
    /// scroll. Returns `true` if a scroll was issued — callers use this to
    /// avoid logging or reacting to reveals that did nothing.
    pub fn scroll_to_reveal(
        &mut self,
        rect: ContentRect,
        margin: RevealMargin,
        behavior: ScrollBehavior,
    ) -> bool {
        let m = self.metrics_now();
        if !m.is_measured() {
            return false;
        }
        let (x, y, w, h) = rect;
        let target_y = reveal_offset(m.scroll_top, m.client_height, m.scroll_height, y, h, margin);
        let target_x = reveal_offset(
            m.scroll_left,
            m.client_width,
            m.scroll_width,
            x,
            w,
            RevealMargin::default(),
        );
        match (target_x, target_y) {
            (None, None) => false,
            (nx, ny) => {
                self.scroll_to(
                    nx.unwrap_or(m.scroll_left),
                    ny.unwrap_or(m.scroll_top),
                    behavior,
                );
                true
            }
        }
    }

    /// Scrolls so that `anchor` keeps showing the same content across a zoom
    /// change from `from_zoom` to `to_zoom` (Spec 08 T5.6).
    ///
    /// `content_origin` is the unscaled offset from the scrollport to the scaled
    /// content — see [`anchored_scroll`].
    ///
    /// # Here rather than at the caller, so the non-subscribing read stays here
    ///
    /// The arithmetic is `zoom_anchor`'s and is tested there. What this adds is
    /// the *reading* of the live geometry, which must go through
    /// [`Self::metrics_now`] — the accessor that does not subscribe (L08-019).
    /// Exposing `metrics_now` so a caller could do this itself would hand out
    /// the one thing the type exists to keep private, and the failure it guards
    /// against is a command path that re-runs on every scroll event.
    ///
    /// Instant, not eased: the content is re-laid-out at the new zoom in the same
    /// frame, so an animated scroll would glide across geometry that has already
    /// changed underneath it.
    ///
    /// Returns `false` when the container is not measured yet, in which case
    /// nothing moved — there is no anchor to hold without a viewport.
    pub fn zoom_to(
        &mut self,
        anchor: ZoomAnchor,
        content_origin: (f32, f32),
        from_zoom: f32,
        to_zoom: f32,
    ) -> bool {
        let m = self.metrics_now();
        if !m.is_measured() {
            return false;
        }
        // `offset_now`, not `m.scroll_*`: a wheel gesture issues several of these
        // per notch and the metrics signal lags each one. See its docs.
        let (x, y) = anchored_scroll(
            self.offset_now(),
            (m.client_width, m.client_height),
            content_origin,
            anchor,
            from_zoom,
            to_zoom,
        );
        self.scroll_to(x, y, ScrollBehavior::Instant);
        true
    }

    /// Scrolls to an absolute offset.
    ///
    /// `Instant` applies immediately. `Smooth` animates unless the motion
    /// preference is [`MotionPreference::Reduced`], in which case it degrades
    /// to instant — the scroll still happens, it just does not animate.
    pub fn scroll_to(&mut self, x: f32, y: f32, behavior: ScrollBehavior) {
        // Every command supersedes an in-flight animation, including an
        // instant one: otherwise a keystroke's instant reveal would be undone
        // by the tail of a smooth scroll still running underneath it.
        self.cancel();
        let smooth = behavior == ScrollBehavior::Smooth && self.motion.peek().animates();
        if !smooth {
            self.apply(x, y);
            return;
        }
        let m = self.metrics_now();
        self.animate(m.scroll_left, m.scroll_top, x, y);
    }

    /// Issues one instant scroll through the mounted container.
    pub(super) fn apply(&self, x: f32, y: f32) {
        let guard = self.mounted.peek();
        let Some(mounted) = guard.as_ref() else {
            return; // container not mounted yet
        };
        // Record what was asked for, here rather than at the callers, so a
        // command path added later cannot forget to (evidence rule 5). Every
        // scroll this type issues — instant, animated tick, reveal — passes
        // through this one function.
        let mut commanded = self.commanded;
        commanded.set(Some((x, y, *self.metrics.peek())));
        // The patched backing performs the scroll eagerly (it posts the event
        // before returning a ready future), so dropping the future here is
        // correct — the same call shape the scrollbar thumb drag uses.
        drop(mounted.scroll(
            PixelsVector2D::new(f64::from(x), f64::from(y)),
            ScrollBehavior::Instant,
        ));
    }
}
