// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Event-driven scroll-settle detector for Dioxus.
//!
//! [`on_scroll_event`] updates the scroll signal and sends the new phase via a
//! watch channel.  [`use_settle_detector`] spawns an async task that debounces
//! scroll activity using [`tokio::time`] and fires `on_settle` exactly once per
//! settle edge.
//!
//! # EVENT-DRIVEN design
//!
// EVENT-DRIVEN: watch channel parks this task until phase changes.
// No 16ms polling loop. The Sender lives in the caller (e.g. RendererState).

use dioxus::prelude::{spawn, use_drop, use_hook, ReadableExt, Signal, WritableExt};
use tokio::sync::watch;

use crate::{ScrollPhase, ScrollState, SETTLE_DURATION};

/// Call from the document scroll handler.
///
/// Updates the scroll signal and sends the phase via `phase_tx` so the settle
/// detector task wakes immediately on scroll activity.
pub fn on_scroll_event(
    mut scroll: Signal<ScrollState>,
    new_top_px: f64,
    phase_tx: &watch::Sender<ScrollPhase>,
) {
    scroll.write().on_scroll(new_top_px);
    let _ = phase_tx.send(scroll.peek().phase);
}

/// Hook: spawn the settle-detector task exactly once per document view.
///
/// `phase_tx` is the caller-owned sender (e.g. `RendererState::phase_tx`) that
/// every [`on_scroll_event`] call posts to. The detector `subscribe()`s to
/// *that same* channel — previously it minted a throwaway channel whose sender
/// was dropped each render, so `on_settle` could never fire (audit F5).
///
/// The spawn is guarded by [`use_hook`] so a single task lives for the
/// component's lifetime instead of one per render, and [`use_drop`] cancels it
/// on unmount. The task debounces scroll-Active signals: after
/// [`SETTLE_DURATION`] elapses without a new Active signal it calls `on_settle`
/// exactly once.
pub fn use_settle_detector(phase_tx: &watch::Sender<ScrollPhase>, on_settle: impl Fn() + 'static) {
    let mut rx = phase_tx.subscribe();

    let task = use_hook(move || {
        spawn(async move {
            // EVENT-DRIVEN: the watch channel parks this task until the phase
            // changes; no polling loop.
            loop {
                if rx.changed().await.is_err() {
                    break;
                }
                if *rx.borrow() != ScrollPhase::Active {
                    continue;
                }
                // Debounce: wait SETTLE_DURATION after the last Active signal.
                loop {
                    match tokio::time::timeout(SETTLE_DURATION, rx.changed()).await {
                        Err(_timeout) => {
                            // No new changes in SETTLE_DURATION → settled.
                            on_settle();
                            break;
                        }
                        Ok(Err(_closed)) => return,
                        Ok(Ok(())) => {
                            if *rx.borrow() != ScrollPhase::Active {
                                break;
                            }
                            // Still Active — restart the debounce window.
                        }
                    }
                }
            }
        })
    });

    use_drop(move || task.cancel());
}
