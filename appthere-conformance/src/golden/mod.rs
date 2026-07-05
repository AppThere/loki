// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Axis 3 — visual goldens (trait + report types).
//!
//! A fixture is rendered by the reference app (the committed golden) and by Loki
//! (the candidate), and the two are compared perceptually. Per Spec 02 **D2**,
//! the candidate render uses an in-process `vello_cpu` software rasterizer so it
//! is deterministic and GPU-free; per **§7.4** the metric is SSIM **plus**
//! CIEDE2000/ΔE, scored **per region** with the *worst* region driving the
//! result, against a **calibrated** threshold (not a guessed `0.98`).
//!
//! This module fixes the report shape and the [`GoldenHarness`] trait. The
//! implementation — the `vello_cpu` candidate render, the ΔE/worst-region
//! differ (promoting `loki-acid`'s SSIM machinery), heatmap emission, and the
//! committed calibration record — is Spec 02 **M5** and intentionally *not*
//! built here. The crate ships the other two axes today; the visual axis is
//! advisory until M5 lands (which is exactly the M1 acceptance bar).

use std::path::PathBuf;

mod ciede;
pub mod diff;

pub use diff::{DiffError, Tolerance, compare_pages, emit_heatmap};

/// A perceptual score for one tiled region of a page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionScore {
    /// Tile coordinates `(col, row)` within the page grid.
    pub region: (u32, u32),
    /// Structural similarity in `[0, 1]` (1 = identical).
    pub ssim: f64,
    /// Perceptual colour delta (CIEDE2000 ΔE); lower is closer.
    pub delta_e: f64,
}

/// The perceptual comparison of one candidate page against its golden.
///
/// A page passes iff **every** region is within tolerance — the worst region,
/// not the mean, drives the result, so a small localized failure is not averaged
/// away (Spec 02 §7.4).
#[derive(Clone, Debug, PartialEq)]
pub struct PerceptualReport {
    /// Per-region scores across the page grid.
    pub regions: Vec<RegionScore>,
    /// The single worst region (lowest SSIM / highest ΔE), if any regions exist.
    pub worst: Option<RegionScore>,
    /// Whether every region met its threshold.
    pub passed: bool,
    /// Optional path to an emitted heatmap-diff PNG for a failure.
    pub heatmap: Option<PathBuf>,
}

/// Errors from the visual-goldens axis (render, golden load, diff).
#[derive(Debug, thiserror::Error)]
pub enum GoldenError {
    /// No committed golden exists for the fixture.
    #[error("no golden committed for fixture '{0}'")]
    MissingGolden(String),
    /// The visual axis is not yet implemented (Spec 02 M5).
    #[error("visual goldens axis not yet implemented (Spec 02 M5): {0}")]
    NotYetImplemented(&'static str),
    /// An I/O error reading a golden or candidate image.
    #[error("visual goldens I/O error: {0}")]
    Io(#[source] std::io::Error),
}

/// Compares a candidate `vello_cpu` render of a fixture against its committed
/// golden, region by region.
///
/// Implemented in Spec 02 M5 (the candidate render, perceptual diff, and
/// calibration). The trait is defined now so consumers and the CI wiring can
/// target a stable shape.
pub trait GoldenHarness {
    /// Compares the candidate render of `fixture_stem` (e.g. `acid_docx`)
    /// against its golden, returning a per-page perceptual report.
    fn compare(&self, fixture_stem: &str) -> Result<Vec<PerceptualReport>, GoldenError>;
}
