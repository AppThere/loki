// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The perceptual differ (Spec 02 §7.4 / B-4): SSIM **and** CIEDE2000 ΔE,
//! scored per tiled region with the **worst region driving the result**, plus
//! failure-heatmap emission.
//!
//! The windowed-SSIM core is promoted from `loki-acid`'s `diff.rs` (M1 —
//! "promote, don't greenfield"); the ΔE metric, regional worst-region
//! scoring, tolerances, and heatmaps are the Spec 02 extensions. Everything
//! is pure Rust over decoded RGBA — no GPU, no external tools.

use image::RgbaImage;

use super::ciede::{ciede2000, srgb_to_lab};
use super::{PerceptualReport, RegionScore};

/// Window side length (pixels) for windowed SSIM.
const WINDOW: u32 = 8;
/// Region (tile) side length in pixels: the granularity at which the worst
/// region is selected. One mis-rendered glyph lands in a 64×64 tile and fails
/// that tile, however large and correct the rest of the page is.
const REGION: u32 = 64;

/// SSIM stabilisation constants on the 0–255 luminance scale.
const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);

/// Error returned when two images cannot be compared.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    /// The two images have different dimensions.
    #[error("dimension mismatch: golden {golden:?} vs candidate {candidate:?}")]
    DimensionMismatch {
        /// Golden image dimensions.
        golden: (u32, u32),
        /// Candidate image dimensions.
        candidate: (u32, u32),
    },
    /// Writing the failure heatmap failed.
    #[error("failed to write heatmap: {0}")]
    Heatmap(#[source] image::ImageError),
}

/// Pass thresholds for one comparison.
///
/// The default comes from the committed calibration record via
/// [`Tolerance::calibrated`] — never a guessed literal (Spec 02 D5). A
/// per-test override must carry a comment justifying it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    /// Minimum acceptable per-region SSIM.
    pub min_ssim: f64,
    /// Maximum acceptable per-region mean CIEDE2000 ΔE.
    pub max_delta_e: f64,
}

impl Tolerance {
    /// The calibrated default thresholds — measured, not guessed (Spec 02
    /// D5). Values and provenance live in
    /// `appthere-conformance/goldens/CALIBRATION.md` and
    /// [`super::calibration`]; update them only together.
    #[must_use]
    pub fn calibrated() -> Self {
        Self {
            min_ssim: super::calibration::CALIBRATED_MIN_SSIM,
            max_delta_e: super::calibration::CALIBRATED_MAX_DELTA_E,
        }
    }
}

/// Converts an RGBA image to a row-major luminance buffer (BT.601, 0–255).
#[must_use]
pub fn to_luma(img: &RgbaImage) -> Vec<f64> {
    img.pixels()
        .map(|p| {
            let [r, g, b, _] = p.0;
            0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)
        })
        .collect()
}

/// SSIM over a single rectangular window of two luminance buffers.
fn window_ssim(la: &[f64], lb: &[f64], stride: u32, x0: u32, y0: u32, w: u32, h: u32) -> f64 {
    let n = f64::from(w * h);
    let (mut sa, mut sb) = (0.0, 0.0);
    for yy in 0..h {
        for xx in 0..w {
            let i = ((y0 + yy) * stride + (x0 + xx)) as usize;
            sa += la[i];
            sb += lb[i];
        }
    }
    let (mu_a, mu_b) = (sa / n, sb / n);
    let (mut va, mut vb, mut cov) = (0.0, 0.0, 0.0);
    for yy in 0..h {
        for xx in 0..w {
            let i = ((y0 + yy) * stride + (x0 + xx)) as usize;
            let (da, db) = (la[i] - mu_a, lb[i] - mu_b);
            va += da * da;
            vb += db * db;
            cov += da * db;
        }
    }
    va /= n;
    vb /= n;
    cov /= n;
    ((2.0 * mu_a * mu_b + C1) * (2.0 * cov + C2))
        / ((mu_a * mu_a + mu_b * mu_b + C1) * (va + vb + C2))
}

/// Compares a candidate page against its golden: every [`REGION`]-sized tile
/// is scored (SSIM via [`WINDOW`]-sized sub-windows, mean CIEDE2000 ΔE per
/// pixel), and the page passes iff **every** region meets `tol`.
pub fn compare_pages(
    golden: &RgbaImage,
    candidate: &RgbaImage,
    tol: Tolerance,
) -> Result<PerceptualReport, DiffError> {
    if golden.dimensions() != candidate.dimensions() {
        return Err(DiffError::DimensionMismatch {
            golden: golden.dimensions(),
            candidate: candidate.dimensions(),
        });
    }
    let (w, h) = golden.dimensions();
    let la = to_luma(golden);
    let lb = to_luma(candidate);

    let mut regions = Vec::new();
    let mut worst: Option<RegionScore> = None;
    let mut passed = true;

    let (cols, rows) = (w.div_ceil(REGION).max(1), h.div_ceil(REGION).max(1));
    for row in 0..rows {
        for col in 0..cols {
            let x0 = col * REGION;
            let y0 = row * REGION;
            let rw = REGION.min(w - x0);
            let rh = REGION.min(h - y0);
            let ssim = region_ssim(&la, &lb, w, x0, y0, rw, rh);
            let delta_e = region_delta_e(golden, candidate, x0, y0, rw, rh);
            let score = RegionScore {
                region: (col, row),
                ssim,
                delta_e,
            };
            let fails = ssim < tol.min_ssim || delta_e > tol.max_delta_e;
            passed &= !fails;
            worst = Some(match worst {
                None => score,
                Some(prev) => {
                    // The "worst" region is the one that violates hardest:
                    // failing beats passing; among equals, lower SSIM, then
                    // higher ΔE.
                    let prev_fails = prev.ssim < tol.min_ssim || prev.delta_e > tol.max_delta_e;
                    let worse = match (fails, prev_fails) {
                        (true, false) => true,
                        (false, true) => false,
                        _ => {
                            score.ssim < prev.ssim
                                || (score.ssim == prev.ssim && score.delta_e > prev.delta_e)
                        }
                    };
                    if worse { score } else { prev }
                }
            });
            regions.push(score);
        }
    }

    Ok(PerceptualReport {
        regions,
        worst,
        passed,
        heatmap: None,
    })
}

/// Mean of the [`WINDOW`]-sized SSIM sub-windows covering one region (windows
/// clipped to the region; a region smaller than one window is one window).
fn region_ssim(la: &[f64], lb: &[f64], stride: u32, x0: u32, y0: u32, w: u32, h: u32) -> f64 {
    if w < WINDOW || h < WINDOW {
        return window_ssim(la, lb, stride, x0, y0, w, h);
    }
    let mut sum = 0.0;
    let mut count = 0u64;
    let mut y = 0;
    while y + WINDOW <= h {
        let mut x = 0;
        while x + WINDOW <= w {
            sum += window_ssim(la, lb, stride, x0 + x, y0 + y, WINDOW, WINDOW);
            count += 1;
            x += WINDOW;
        }
        y += WINDOW;
    }
    if count == 0 { 1.0 } else { sum / count as f64 }
}

/// Mean per-pixel CIEDE2000 ΔE over one region.
fn region_delta_e(a: &RgbaImage, b: &RgbaImage, x0: u32, y0: u32, w: u32, h: u32) -> f64 {
    let mut sum = 0.0;
    for yy in 0..h {
        for xx in 0..w {
            let pa = a.get_pixel(x0 + xx, y0 + yy).0;
            let pb = b.get_pixel(x0 + xx, y0 + yy).0;
            sum += ciede2000(srgb_to_lab(pa), srgb_to_lab(pb));
        }
    }
    sum / f64::from(w * h)
}

/// Emits a failure heatmap: the golden as a dimmed grayscale base with
/// per-pixel ΔE painted red (intensity ∝ ΔE, saturating at ΔE = 20).
pub fn emit_heatmap(
    golden: &RgbaImage,
    candidate: &RgbaImage,
    path: &std::path::Path,
) -> Result<(), DiffError> {
    let (w, h) = golden.dimensions();
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let pg = golden.get_pixel(x, y).0;
            let pc = candidate.get_pixel(x, y).0;
            let de = ciede2000(srgb_to_lab(pg), srgb_to_lab(pc));
            let luma =
                (0.299 * f64::from(pg[0]) + 0.587 * f64::from(pg[1]) + 0.114 * f64::from(pg[2]))
                    * 0.5;
            let heat = (de / 20.0).min(1.0);
            let r = (luma + (255.0 - luma) * heat) as u8;
            let gb = (luma * (1.0 - heat)) as u8;
            out.put_pixel(x, y, image::Rgba([r, gb, gb, 255]));
        }
    }
    out.save(path).map_err(DiffError::Heatmap)
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
