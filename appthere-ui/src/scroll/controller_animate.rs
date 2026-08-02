// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Driving a smooth scroll to completion (Spec 08 T1.1).
//!
//! Split out of `controller.rs` for the 300-line ceiling, and it is the right
//! seam: everything here is about *time* — a worker thread, a channel, and an
//! elapsed clock — where the rest of the controller is about geometry.
//!
//! The easing curve itself is [`super::animate`]'s and is tested there. What
//! lives here is the tick source, which exists because there is no async timer
//! under `dioxus-native`.

use std::time::{Duration, Instant};

use dioxus::prelude::*;

use super::animate::{animation_step, SMOOTH_DURATION_MS, TICK_MS};
use super::controller::ViewportController;

impl ViewportController {
    /// Drives a smooth scroll from `(fx, fy)` to `(tx, ty)`.
    ///
    /// Ticks arrive from a worker thread through a channel — there is no async
    /// timer under `dioxus-native`. The loop exits early the moment
    /// `generation` moves, so a superseding command takes effect on the next
    /// tick rather than after this animation finishes.
    pub(super) fn animate(&mut self, fx: f32, fy: f32, tx: f32, ty: f32) {
        let me = *self;
        let epoch = self.generation_now();
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
                if me.generation_now() != epoch {
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
