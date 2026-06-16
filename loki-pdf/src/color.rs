// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Colour conversion for the print (CMYK) PDF/X pipeline.
//!
//! All supported PDF/X levels are valid with a CMYK-only colour workflow, so
//! every layout colour is converted to DeviceCMYK before it is written. The
//! conversion is the standard naive RGB→CMYK transform (no ICC colour
//! management); for colour-managed workflows an ICC `DestOutputProfile` should
//! be supplied via [`crate::OutputIntent`].

use loki_layout::LayoutColor;

/// A DeviceCMYK colour with components in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cmyk {
    /// Cyan.
    pub c: f32,
    /// Magenta.
    pub m: f32,
    /// Yellow.
    pub y: f32,
    /// Key (black).
    pub k: f32,
}

impl Cmyk {
    /// Pure black, used for the common case of black text.
    pub const BLACK: Cmyk = Cmyk {
        c: 0.0,
        m: 0.0,
        y: 0.0,
        k: 1.0,
    };
}

/// Converts an sRGB layout colour to DeviceCMYK using the naive transform.
///
/// `k = 1 − max(r,g,b)`, then each chromatic channel is the complement scaled
/// by the remaining luminance. Fully transparent or near-white inputs map to
/// all-zero ink. Alpha is ignored — the print pipeline is opaque.
#[must_use]
pub fn layout_to_cmyk(color: LayoutColor) -> Cmyk {
    let (r, g, b) = (
        color.r.clamp(0.0, 1.0),
        color.g.clamp(0.0, 1.0),
        color.b.clamp(0.0, 1.0),
    );
    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 - f32::EPSILON {
        // Pure black: avoid a divide-by-zero and emit clean K-only ink.
        return Cmyk::BLACK;
    }
    let denom = 1.0 - k;
    Cmyk {
        c: (1.0 - r - k) / denom,
        m: (1.0 - g - k) / denom,
        y: (1.0 - b - k) / denom,
        k,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_maps_to_k_only() {
        let cmyk = layout_to_cmyk(LayoutColor::BLACK);
        assert_eq!(cmyk, Cmyk::BLACK);
    }

    #[test]
    fn white_maps_to_no_ink() {
        let cmyk = layout_to_cmyk(LayoutColor::WHITE);
        assert_eq!(cmyk.k, 0.0);
        assert_eq!(cmyk.c, 0.0);
        assert_eq!(cmyk.m, 0.0);
        assert_eq!(cmyk.y, 0.0);
    }

    #[test]
    fn pure_red_is_magenta_plus_yellow() {
        let cmyk = layout_to_cmyk(LayoutColor::new(1.0, 0.0, 0.0, 1.0));
        assert!(cmyk.c.abs() < 1e-6);
        assert!((cmyk.m - 1.0).abs() < 1e-6);
        assert!((cmyk.y - 1.0).abs() < 1e-6);
        assert!(cmyk.k.abs() < 1e-6);
    }
}
