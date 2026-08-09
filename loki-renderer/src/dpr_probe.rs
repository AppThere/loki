// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The display scale factor the paint path is actually rendering at
//! (Spec 08 R27, T2.0).
//!
//! # Why this is observed rather than queried
//!
//! Blitz owns the factor and hands it to the paint source as
//! [`CustomPaintSource::render`]'s `scale` argument. Nothing surfaces it to the
//! application, and the tile planner needs it *before* the render callback in
//! order to decide what to mount and at what rasterisation scale. So it is
//! recorded on the way past, exactly as [`crate::gpu_probe`] records the adapter
//! class: the value is a property of the window, not of a tile, so one
//! process-wide cell is the whole fact rather than N copies of it.
//!
//! # The one-frame lag, and why it is the right direction
//!
//! Before the first page tile paints there is nothing to observe, so
//! [`observed_scale`] returns `None` and the planner falls back to 1.0. That
//! under-states the texture cost on a HiDPI display for exactly one frame, after
//! which the application lifts the real value into its `DeviceProfile` and every
//! subsequent plan is correct.
//!
//! Under-stating over-mounts; it never evicts. So the lag costs a frame of extra
//! residency and can never blank a page — the same benign direction `gpu_probe`
//! already accepts for the adapter class. `None` is deliberately distinct from
//! `Some(1.0)`: "not yet observed" must not read as "standard-DPI display", or a
//! genuine 1.0 display and an unprobed 3x display would be indistinguishable.
//!
//! # Why a scale *change* matters and not just its first value
//!
//! Dragging a window between a Retina and an external display changes the factor
//! mid-session, and the rasterisation scale is part of a tile's invalidation key
//! (`tile_key`), so a stale factor would keep serving textures cut for the old
//! display. Recording every frame rather than only the first means the observed
//! value tracks the window, and the application's sensor sees the change on its
//! next poll.

use std::sync::atomic::{AtomicU64, Ordering};

/// Last scale factor the paint path rendered at, as `f64` bits. Zero means
/// nothing has painted yet.
static OBSERVED: AtomicU64 = AtomicU64::new(0);

/// The scale factor the paint path last rendered at, or `None` before the first
/// paint.
///
/// A non-finite or non-positive observation is discarded rather than returned: it
/// would be a bug upstream, and propagating it would turn a wrong number into a
/// gigantic or zero-sized allocation.
#[must_use]
pub fn observed_scale() -> Option<f64> {
    let raw = OBSERVED.load(Ordering::Relaxed);
    if raw == 0 {
        return None;
    }
    let v = f64::from_bits(raw);
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Records the scale factor a page tile was just rendered at.
pub(crate) fn record(scale: f64) {
    if scale.is_finite() && scale > 0.0 {
        OBSERVED.store(scale.to_bits(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::{observed_scale, record};

    /// One test: the cell is a process-wide static and `cargo test` is threaded,
    /// so separate functions touching it would interleave.
    #[test]
    fn records_observes_and_rejects_nonsense() {
        assert_eq!(observed_scale(), None, "nothing painted yet");

        record(2.0);
        assert_eq!(observed_scale(), Some(2.0));

        // A later frame on a different display supersedes it — the factor is a
        // property of the window and windows move between displays.
        record(3.0);
        assert_eq!(observed_scale(), Some(3.0));

        // Nonsense is discarded rather than stored, so the last good value
        // stands instead of a zero-sized or non-finite texture request.
        record(0.0);
        record(-1.0);
        record(f64::NAN);
        record(f64::INFINITY);
        assert_eq!(observed_scale(), Some(3.0));
    }
}
