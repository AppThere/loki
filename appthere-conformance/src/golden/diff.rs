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
/// The default threshold must come from the committed calibration record
/// (Spec 02 D5) — never a guessed literal. Until that record lands (M5
/// calibration pass), construct explicitly; a per-test override must carry a
/// comment justifying it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    /// Minimum acceptable per-region SSIM.
    pub min_ssim: f64,
    /// Maximum acceptable per-region mean CIEDE2000 ΔE.
    pub max_delta_e: f64,
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

// ── sRGB → CIELAB (D65) and CIEDE2000 ────────────────────────────────────────

/// Converts an 8-bit sRGB pixel to CIELAB (D65 white point).
fn srgb_to_lab([r, g, b, _]: [u8; 4]) -> [f64; 3] {
    fn linear(c: u8) -> f64 {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let (rl, gl, bl) = (linear(r), linear(g), linear(b));
    // sRGB D65 → XYZ.
    let x = 0.4124564 * rl + 0.3575761 * gl + 0.1804375 * bl;
    let y = 0.2126729 * rl + 0.7151522 * gl + 0.0721750 * bl;
    let z = 0.0193339 * rl + 0.1191920 * gl + 0.9503041 * bl;
    // XYZ → Lab with the D65 reference white.
    fn f(t: f64) -> f64 {
        const DELTA: f64 = 6.0 / 29.0;
        if t > DELTA * DELTA * DELTA {
            t.cbrt()
        } else {
            t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
        }
    }
    let (xn, yn, zn) = (0.95047, 1.0, 1.08883);
    let (fx, fy, fz) = (f(x / xn), f(y / yn), f(z / zn));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 colour difference (Sharma et al. 2005 formulation).
#[allow(clippy::many_single_char_names, clippy::similar_names)]
fn ciede2000([l1, a1, b1]: [f64; 3], [l2, a2, b2]: [f64; 3]) -> f64 {
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let c_bar = (c1 + c2) / 2.0;
    let c_bar7 = c_bar.powi(7);
    let g = 0.5 * (1.0 - (c_bar7 / (c_bar7 + 25.0_f64.powi(7))).sqrt());
    let ap1 = (1.0 + g) * a1;
    let ap2 = (1.0 + g) * a2;
    let cp1 = (ap1 * ap1 + b1 * b1).sqrt();
    let cp2 = (ap2 * ap2 + b2 * b2).sqrt();
    let hp = |ap: f64, b: f64| -> f64 {
        if ap == 0.0 && b == 0.0 {
            0.0
        } else {
            let h = b.atan2(ap).to_degrees();
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let hp1 = hp(ap1, b1);
    let hp2 = hp(ap2, b2);

    let dl = l2 - l1;
    let dc = cp2 - cp1;
    let dhp = if cp1 * cp2 == 0.0 {
        0.0
    } else {
        let d = hp2 - hp1;
        if d.abs() <= 180.0 {
            d
        } else if d > 180.0 {
            d - 360.0
        } else {
            d + 360.0
        }
    };
    let dh = 2.0 * (cp1 * cp2).sqrt() * (dhp.to_radians() / 2.0).sin();

    let l_bar = (l1 + l2) / 2.0;
    let cp_bar = (cp1 + cp2) / 2.0;
    let hp_bar = if cp1 * cp2 == 0.0 {
        hp1 + hp2
    } else {
        let sum = hp1 + hp2;
        let d = (hp1 - hp2).abs();
        if d <= 180.0 {
            sum / 2.0
        } else if sum < 360.0 {
            (sum + 360.0) / 2.0
        } else {
            (sum - 360.0) / 2.0
        }
    };

    let t = 1.0 - 0.17 * (hp_bar - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hp_bar).to_radians().cos()
        + 0.32 * (3.0 * hp_bar + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hp_bar - 63.0).to_radians().cos();
    let d_theta = 30.0 * (-((hp_bar - 275.0) / 25.0).powi(2)).exp();
    let cp_bar7 = cp_bar.powi(7);
    let rc = 2.0 * (cp_bar7 / (cp_bar7 + 25.0_f64.powi(7))).sqrt();
    let lb50 = (l_bar - 50.0).powi(2);
    let sl = 1.0 + 0.015 * lb50 / (20.0 + lb50).sqrt();
    let sc = 1.0 + 0.045 * cp_bar;
    let sh = 1.0 + 0.015 * cp_bar * t;
    let rt = -(2.0 * d_theta).to_radians().sin() * rc;

    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dh / sh).powi(2) + rt * (dc / sc) * (dh / sh)).sqrt()
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
