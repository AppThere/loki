// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Perceptual image-diff primitives for the golden pixel comparison.
//!
//! The plan calls for "a perceptual diff (SSIM) plus a hard glyph-coverage
//! check". Glyph coverage lives in [`crate::pages`]; this module provides the
//! SSIM and absolute-difference metrics over decoded RGBA images. They are pure
//! and unit-tested so they are ready the moment golden references exist.

use image::RgbaImage;

/// Window side length (pixels) for windowed SSIM.
const WINDOW: u32 = 8;

/// SSIM stabilisation constants on the 0–255 luminance scale.
const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);

/// Error returned when two images cannot be compared.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    /// The two images have different dimensions.
    #[error("dimension mismatch: {a:?} vs {b:?}")]
    DimensionMismatch {
        /// First image dimensions.
        a: (u32, u32),
        /// Second image dimensions.
        b: (u32, u32),
    },
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

/// Returns the mean structural-similarity index between two equally-sized
/// images, in `[-1.0, 1.0]` (1.0 = identical).
pub fn mean_ssim(a: &RgbaImage, b: &RgbaImage) -> Result<f64, DiffError> {
    if a.dimensions() != b.dimensions() {
        return Err(DiffError::DimensionMismatch {
            a: a.dimensions(),
            b: b.dimensions(),
        });
    }
    let (w, h) = a.dimensions();
    let la = to_luma(a);
    let lb = to_luma(b);

    // Degenerate images smaller than one window: a single global window.
    if w < WINDOW || h < WINDOW {
        return Ok(window_ssim(&la, &lb, w, 0, 0, w, h));
    }

    let mut sum = 0.0;
    let mut count = 0u64;
    let mut y = 0;
    while y + WINDOW <= h {
        let mut x = 0;
        while x + WINDOW <= w {
            sum += window_ssim(&la, &lb, w, x, y, WINDOW, WINDOW);
            count += 1;
            x += WINDOW;
        }
        y += WINDOW;
    }
    Ok(if count == 0 { 1.0 } else { sum / count as f64 })
}

/// SSIM over a single rectangular window.
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

/// Fraction of pixels whose per-channel luminance differs by more than
/// `threshold` (0–255), in `[0.0, 1.0]`.
pub fn abs_diff_ratio(a: &RgbaImage, b: &RgbaImage, threshold: f64) -> Result<f64, DiffError> {
    if a.dimensions() != b.dimensions() {
        return Err(DiffError::DimensionMismatch {
            a: a.dimensions(),
            b: b.dimensions(),
        });
    }
    let la = to_luma(a);
    let lb = to_luma(b);
    let differing = la
        .iter()
        .zip(&lb)
        .filter(|(x, y)| (*x - *y).abs() > threshold)
        .count();
    Ok(if la.is_empty() {
        0.0
    } else {
        differing as f64 / la.len() as f64
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(c))
    }

    #[test]
    fn identical_images_have_ssim_one() {
        let img = solid(16, 16, [120, 130, 140, 255]);
        let ssim = mean_ssim(&img, &img).unwrap();
        assert!((ssim - 1.0).abs() < 1e-9, "ssim={ssim}");
        assert_eq!(abs_diff_ratio(&img, &img, 1.0).unwrap(), 0.0);
    }

    #[test]
    fn black_vs_white_has_low_ssim() {
        let a = solid(16, 16, [0, 0, 0, 255]);
        let b = solid(16, 16, [255, 255, 255, 255]);
        let ssim = mean_ssim(&a, &b).unwrap();
        assert!(ssim < 0.05, "ssim={ssim}");
        assert_eq!(abs_diff_ratio(&a, &b, 1.0).unwrap(), 1.0);
    }

    #[test]
    fn dimension_mismatch_errors() {
        let a = solid(8, 8, [0, 0, 0, 255]);
        let b = solid(9, 8, [0, 0, 0, 255]);
        assert!(mean_ssim(&a, &b).is_err());
    }
}
