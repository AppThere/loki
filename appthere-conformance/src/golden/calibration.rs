// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The committed calibration constants (Spec 02 D5).
//!
//! These values are **measured, not guessed** — they derive from the
//! calibration record at `appthere-conformance/goldens/CALIBRATION.md`
//! (2026-07-05: LibreOffice 24.2 goldens vs the `vello_cpu` candidate over
//! the M4 baseline fixture set; thresholds set with margin above the noise
//! floor of the agreeing fixtures). Change them only together with that
//! record, by re-running `cargo run -p loki-render-cpu --example
//! calibrate_odf`.

/// Minimum per-region SSIM (measured floor on correct fixtures: 0.6324).
pub const CALIBRATED_MIN_SSIM: f64 = 0.60;

/// Maximum per-region mean CIEDE2000 ΔE (measured max on correct fixtures:
/// 9.083).
pub const CALIBRATED_MAX_DELTA_E: f64 = 10.0;
