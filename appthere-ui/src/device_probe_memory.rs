// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Keeping the memory figure *observed* rather than sampled once (Spec 08 T1.6).
//!
//! # The contract this restores
//!
//! T1.6 specifies every `DeviceProfile` probe as "observable, not sampled once,
//! since a keyboard or mouse can arrive mid-session", and names memory in that
//! list. `DeviceProbeSensor` read it in a `use_hook` — exactly once, at mount —
//! and justified it with "re-reading it per frame would make the budget jitter
//! with whatever else the machine is doing".
//!
//! The objection to per-frame is right; the conclusion does not follow, because
//! per-frame and once are not the only options. And once-at-mount undercuts the
//! reason the derivation prefers *available* RAM over total in the first place:
//! the point of `MemAvailable` is to be a good citizen under pressure the app
//! cannot see at startup. A figure read at launch describes a moment when this
//! process is about to allocate and the system is often at its busiest — close
//! to the worst available sampling instant, and then frozen for the session.
//!
//! # Two knobs, and why the second is quantisation rather than hysteresis
//!
//! - [`MEMORY_RESAMPLE_SECS`] bounds how stale the figure can be.
//! - [`MEMORY_QUANTUM_BYTES`] decides what counts as a *material* change.
//!
//! The obvious choice for the second is relative hysteresis — ignore a move
//! smaller than some fraction of the stored value. It has a ratchet failure:
//! a machine drifting from 8 GiB available to 2 GiB in steps each below the
//! threshold never updates at all, which is precisely the scenario the budget
//! exists for. Quantising to a fixed grid has no such hole — drift crosses a
//! bucket boundary eventually, whatever step size it takes — while jitter inside
//! one bucket still writes nothing and wakes nobody.
//!
//! 256 MiB of available RAM is 4 MiB of texture budget at the `/64` divisor, so
//! the grid is fine enough that quantisation never costs a visible amount of
//! budget and coarse enough that ordinary churn is invisible.

use std::time::Duration;

use dioxus::prelude::*;

use crate::device_probe::{note_system_memory, probe_system_memory, SystemMemory};

/// How often `/proc/meminfo` is re-read, in seconds.
///
/// Long enough that the read itself is free, short enough that a machine coming
/// under pressure is noticed within a few seconds rather than never. Memory
/// pressure changes far more often than the input devices T1.6 was written for.
pub const MEMORY_RESAMPLE_SECS: u64 = 5;

/// Grid the available-RAM figure is snapped to before it reaches the profile.
///
/// See the module docs for why this is a grid and not a percentage.
pub const MEMORY_QUANTUM_BYTES: u64 = 256 * 1024 * 1024;

/// Rounds a byte figure to the nearest [`MEMORY_QUANTUM_BYTES`], never to zero.
///
/// Never to zero because a machine genuinely down to its last few MiB is the one
/// case where the budget most needs to react, and reporting `0` there would read
/// as "no probe" to a consumer that treats zero as absent.
#[must_use]
pub fn quantise_bytes(bytes: u64) -> u64 {
    let half = MEMORY_QUANTUM_BYTES / 2;
    let snapped = ((bytes + half) / MEMORY_QUANTUM_BYTES) * MEMORY_QUANTUM_BYTES;
    snapped.max(MEMORY_QUANTUM_BYTES.min(bytes.max(1)))
}

/// A memory observation with its figures snapped to the materiality grid.
#[must_use]
pub fn quantised(observed: SystemMemory) -> SystemMemory {
    SystemMemory {
        // Total is quantised too, so the two paths through the derivation move
        // for the same reasons. It should never change at all; if it does, the
        // machine has been resized under us and a 256 MiB grid is not the thing
        // that will mislead anyone.
        total_bytes: observed.total_bytes.map(quantise_bytes),
        available_bytes: observed.available_bytes.map(quantise_bytes),
    }
}

/// Starts the repeating memory probe. Call once, from the device sensor.
///
/// A worker thread sleeps and sends each observation through a channel, which
/// the Dioxus side awaits — the cross-thread yield idiom already used for the
/// save-status auto-clear and the scroll animator, because signals may only be
/// touched from the runtime's own task.
///
/// The thread is detached and runs for the process lifetime. That is deliberate
/// rather than lazy: the sensor is mounted once at the app root and lives as
/// long as the window, so a shutdown handshake would add a channel and a
/// join for a thread whose whole body is a sleep and a file read.
pub fn use_memory_resampling() {
    use_hook(|| {
        // Seed synchronously so the first frame has a budget derived from this
        // machine rather than from the baseline — the resample loop's first tick
        // is a whole interval away.
        note_system_memory(quantised(probe_system_memory()));

        let (tx, mut rx) = futures_channel::mpsc::unbounded::<SystemMemory>();
        let spawned = std::thread::Builder::new()
            .name("loki-memory-probe".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(MEMORY_RESAMPLE_SECS));
                    // A closed channel means the runtime is gone; stop rather
                    // than spin for the rest of the process's life.
                    if tx.unbounded_send(probe_system_memory()).is_err() {
                        return;
                    }
                }
            });
        if spawned.is_ok() {
            spawn(async move {
                use futures_util::StreamExt;
                while let Some(observed) = rx.next().await {
                    // `note_system_memory` writes only on a change, so an
                    // unchanged bucket wakes no consumer.
                    note_system_memory(quantised(observed));
                }
            });
        }
    });
}

#[cfg(test)]
#[path = "device_probe_memory_tests.rs"]
mod tests;
