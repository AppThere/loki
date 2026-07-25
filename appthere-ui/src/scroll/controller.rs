// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`ViewportController`] — the one handle on a scroll container (Spec 08 T1.2).

use std::time::{Duration, Instant};

use dioxus::html::geometry::PixelsVector2D;
use dioxus::prelude::*;

use super::animate::{animation_step, MotionPreference, SMOOTH_DURATION_MS, TICK_MS};
use super::metrics::ScrollMetrics;
use super::reveal::{reveal_offset, RevealMargin};

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
/// band around the caret, and the cap was asymmetric (one line up, three down)
/// because the margin is.
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
    ViewportController {
        metrics,
        mounted,
        generation,
        motion,
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
    fn apply(&self, x: f32, y: f32) {
        let guard = self.mounted.peek();
        let Some(mounted) = guard.as_ref() else {
            return; // container not mounted yet
        };
        // The patched backing performs the scroll eagerly (it posts the event
        // before returning a ready future), so dropping the future here is
        // correct — the same call shape the scrollbar thumb drag uses.
        drop(mounted.scroll(
            PixelsVector2D::new(f64::from(x), f64::from(y)),
            ScrollBehavior::Instant,
        ));
    }

    /// Drives a smooth scroll from `(fx, fy)` to `(tx, ty)`.
    ///
    /// Ticks arrive from a worker thread through a channel — there is no async
    /// timer under `dioxus-native` (see the [`super::animate`] module docs).
    /// The loop exits early the moment `generation` moves, so a superseding
    /// command takes effect on the next tick rather than after this animation
    /// finishes.
    fn animate(&mut self, fx: f32, fy: f32, tx: f32, ty: f32) {
        let me = *self;
        let epoch = *self.generation.peek();
        let (tick_tx, mut tick_rx) = futures_channel::mpsc::unbounded::<()>();
        let spawned = std::thread::Builder::new()
            .name("at-scroll-anim".into())
            .spawn(move || {
                // Bounded by construction: duration / interval + slack.
                let ticks = (SMOOTH_DURATION_MS as u64 / TICK_MS) + 2;
                for _ in 0..ticks {
                    std::thread::sleep(Duration::from_millis(TICK_MS));
                    if tick_tx.unbounded_send(()).is_err() {
                        break; // receiver dropped — animation superseded
                    }
                }
            });
        if spawned.is_err() {
            me.apply(tx, ty); // no thread available: land on the target anyway
            return;
        }
        let start = Instant::now();
        spawn(async move {
            use futures_util::StreamExt;
            while tick_rx.next().await.is_some() {
                if *me.generation.peek() != epoch {
                    return; // superseded or cancelled
                }
                let elapsed = start.elapsed().as_secs_f32() * 1000.0;
                let (x, done) = animation_step(fx, tx, elapsed, SMOOTH_DURATION_MS);
                let (y, _) = animation_step(fy, ty, elapsed, SMOOTH_DURATION_MS);
                me.apply(x, y);
                if done {
                    return;
                }
            }
        });
    }
}
