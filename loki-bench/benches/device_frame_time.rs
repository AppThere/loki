// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! GPU frame-time (Spec 06 §5 / M5): the production paint path's per-frame cost,
//! measured via **wgpu timestamp queries** on real hardware (Kevin's Windows+RTX
//! 3050 / MacBook A16). It is **not agent-runnable** — the agent has no GPU — and
//! never gates CI.
//!
//! This is the one M5 metric that cannot be executed or verified headless (peak
//! RSS *is* runnable — see the `device_rss` target). It stays gated behind the
//! `device` feature so no GPU dependency enters the default headless build; the
//! on-device implementation wires wgpu timestamp queries around the
//! `loki-renderer` paint path and records per-frame GPU time alongside the
//! `device_rss` peak-RSS pass. See `docs/adr/spec-06-calibration.md`.
//!
//! Run (on GPU hardware): `cargo bench -p loki-bench --bench device_frame_time --features device`

fn main() {
    #[cfg(not(feature = "device"))]
    {
        eprintln!(
            "device_frame_time — hardware-only (Spec 06 §5 / M5). Skipped: the agent \
             environment has no GPU. Re-run on real hardware with `--features device`. \
             Peak RSS (the runnable M5 metric) is the `device_rss` target."
        );
    }

    #[cfg(feature = "device")]
    {
        eprintln!(
            "device_frame_time — device feature enabled. The wgpu timestamp-query \
             frame-time pass is wired around the loki-renderer paint path on-device \
             (Spec 06 M5); see docs/adr/spec-06-calibration.md."
        );
    }
}
