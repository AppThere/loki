// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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

use dioxus::dioxus_core::Task;
use dioxus::prelude::{spawn, ReadableExt, Signal, WritableExt};
use tokio::sync::watch;

use crate::scroll::{ScrollPhase, ScrollState, SETTLE_DURATION};

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

/// Spawn once per document view.
///
/// Returns `(task, phase_tx)`.  Pass `phase_tx` to every [`on_scroll_event`]
/// call.  The spawned task debounces scroll-Active signals: after
/// [`SETTLE_DURATION`] elapses without a new Active signal it calls `on_settle`
/// exactly once.
///
/// Cancel the task on component unmount:
/// ```ignore
/// use_drop(move || task.cancel());
/// ```
pub fn use_settle_detector(
    _scroll: Signal<ScrollState>,
    on_settle: impl Fn() + 'static,
) -> (Task, watch::Sender<ScrollPhase>) {
    let (tx, mut rx) = watch::channel(ScrollPhase::Idle);

    let task = spawn(async move {
        // EVENT-DRIVEN: watch channel parks this task until phase changes.
        // No 16ms polling loop. The Sender lives in the caller (e.g. RendererState).
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            if *rx.borrow() != ScrollPhase::Active {
                continue;
            }
            // Debounce: wait SETTLE_DURATION after last Active signal.
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
    });

    (task, tx)
}
