// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Device-bound axis marker (Spec 06 §5): GPU frame-time and real peak RSS run
//! **only on real hardware** (Kevin's Windows+RTX 3050 / MacBook A16) — they are
//! not agent-runnable and never gate CI.
//!
//! M1 stakes out the axis without pulling GPU dependencies into the default,
//! headless build: the target compiles and runs everywhere, but the actual
//! measurement is gated behind the `device` feature and lands in Spec 06 M5.
//! Running it headless is a deliberate, explicit no-op with a hardware notice.
//!
//! Run (on GPU hardware): `cargo bench -p loki-bench --bench device_frame_time --features device`

fn main() {
    #[cfg(not(feature = "device"))]
    {
        eprintln!(
            "device_frame_time — hardware-only (Spec 06 §5). Skipped: the agent \
             environment has no GPU. Re-run on real hardware with `--features \
             device`. GPU frame-time + peak RSS are implemented in Spec 06 M5."
        );
    }

    #[cfg(feature = "device")]
    {
        eprintln!(
            "device_frame_time — device feature enabled; GPU frame-time + peak \
             RSS measurement is Spec 06 M5 (not yet implemented)."
        );
    }
}
