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
    /// The live scroll geometry.
    #[must_use]
    pub fn metrics(&self) -> ScrollMetrics {
        *self.metrics.read()
    }

    /// The visible region in content coordinates, `(x, y, width, height)`.
    #[must_use]
    pub fn visible_rect(&self) -> ContentRect {
        self.metrics.read().visible_rect()
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
        let m = self.metrics();
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
        let smooth = behavior == ScrollBehavior::Smooth && self.motion.read().animates();
        if !smooth {
            self.apply(x, y);
            return;
        }
        let m = self.metrics();
        self.animate(m.scroll_left, m.scroll_top, x, y);
    }

    /// Issues one instant scroll through the mounted container.
    fn apply(&self, x: f32, y: f32) {
        let guard = self.mounted.read();
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
