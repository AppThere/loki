// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What GPU is actually painting (Spec 08 T2.0).
//!
//! # Why the observation lives here and the profile does not
//!
//! `appthere_ui::DeviceProfile` is where the answer belongs, but that crate is
//! L5 and this one is L4 — an edge from here to it is uphill and the
//! dependency-direction gate refuses it (ADR-0009; the previous
//! `loki-renderer → appthere-ui` edge was removed by Spec 01 audit A-8). So
//! this module records the observation in a neutral form and the **application**
//! (L6, which depends on both) maps it into the profile.
//!
//! # Why not enumerate adapters instead
//!
//! Creating a second `wgpu::Instance` to ask what adapters exist would answer a
//! different question. Blitz chooses the adapter; a phase that sizes a texture
//! budget needs the class of the adapter *in use*, not the best one available.
//! `wgpu_context::DeviceHandle` hands us exactly that at `resume`, for free.
//!
//! # Observable, not sampled
//!
//! The adapter is known at first paint, not at process start, so this is a cell
//! that fills in later rather than a value read once at launch. A consumer polls
//! it — see the application-side sensor — and the profile stays `Unknown` until
//! the GPU path has actually resumed. That is the honest state: before the first
//! paint there is no adapter to describe.

use std::sync::atomic::{AtomicU8, Ordering};

/// The adapter classes `wgpu::DeviceType` distinguishes, without naming wgpu in
/// the signature so consumers need no wgpu dependency of their own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AdapterKind {
    /// Discrete GPU with its own memory.
    Discrete,
    /// Integrated GPU sharing system memory.
    Integrated,
    /// Paravirtualised GPU (a VM guest, or a hosted Android image with GPU
    /// passthrough). Real hardware underneath, so it is treated as capable.
    Virtual,
    /// Software rasteriser — SwiftShader on the Android emulator, llvmpipe on a
    /// headless Linux box. Cannot run Vello's compute pipelines.
    Cpu,
    /// The backend reported something else.
    Other,
}

/// Sentinel for "no adapter observed yet". Distinct from every real class, so a
/// consumer can tell *not looked yet* from *looked and found nothing* (L9-009).
const UNOBSERVED: u8 = 0;

static OBSERVED: AtomicU8 = AtomicU8::new(UNOBSERVED);

fn encode(kind: AdapterKind) -> u8 {
    match kind {
        AdapterKind::Discrete => 1,
        AdapterKind::Integrated => 2,
        AdapterKind::Virtual => 3,
        AdapterKind::Cpu => 4,
        AdapterKind::Other => 5,
    }
}

fn decode(raw: u8) -> Option<AdapterKind> {
    match raw {
        1 => Some(AdapterKind::Discrete),
        2 => Some(AdapterKind::Integrated),
        3 => Some(AdapterKind::Virtual),
        4 => Some(AdapterKind::Cpu),
        5 => Some(AdapterKind::Other),
        _ => None,
    }
}

/// The adapter class the paint path resumed on, or `None` before the first
/// paint.
#[must_use]
pub fn observed_adapter() -> Option<AdapterKind> {
    decode(OBSERVED.load(Ordering::Relaxed))
}

/// Records the adapter class the paint path resumed on.
///
/// Process-wide because the adapter is: every page tile in every tab resumes on
/// the same one, so a per-source record would be N copies of one fact.
pub(crate) fn record(kind: AdapterKind) {
    OBSERVED.store(encode(kind), Ordering::Relaxed);
}

/// Maps a wgpu adapter's reported device type onto [`AdapterKind`].
pub(crate) fn kind_of(device_type: anyrender_vello::wgpu::DeviceType) -> AdapterKind {
    use anyrender_vello::wgpu::DeviceType;
    match device_type {
        DeviceType::DiscreteGpu => AdapterKind::Discrete,
        DeviceType::IntegratedGpu => AdapterKind::Integrated,
        DeviceType::VirtualGpu => AdapterKind::Virtual,
        DeviceType::Cpu => AdapterKind::Cpu,
        DeviceType::Other => AdapterKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::{AdapterKind, decode, encode, observed_adapter, record};

    /// One test: the cell is a process-wide static and `cargo test` is
    /// threaded, so separate functions touching it would interleave. Same
    /// reasoning as the residency counter's tests.
    #[test]
    fn the_cell_round_trips_and_starts_unobserved() {
        // Before anything is recorded, "not looked yet" is distinguishable from
        // every real class — which is the whole point of the sentinel.
        assert_eq!(decode(0), None);

        for kind in [
            AdapterKind::Discrete,
            AdapterKind::Integrated,
            AdapterKind::Virtual,
            AdapterKind::Cpu,
            AdapterKind::Other,
        ] {
            assert_eq!(decode(encode(kind)), Some(kind));
            record(kind);
            assert_eq!(observed_adapter(), Some(kind));
        }
    }
}
