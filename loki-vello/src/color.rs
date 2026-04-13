// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Conversion utilities from [`loki_layout::LayoutColor`] to `peniko` color types.
//!
//! Both representations use `f32` RGBA components in `[0.0, 1.0]`, so
//! conversion is a direct component copy with no clamping required.

use loki_layout::LayoutColor;

/// Convert a [`LayoutColor`] to a [`peniko::Color`] (`AlphaColor<Srgb>`).
///
/// Both types use linear `f32` components in `[0.0, 1.0]`, so this is a
/// zero-cost component copy.
pub fn to_peniko(c: &LayoutColor) -> peniko::Color {
    peniko::Color::new([c.r, c.g, c.b, c.a])
}

/// Convert a [`LayoutColor`] to a solid [`peniko::Brush`].
pub fn to_brush(c: &LayoutColor) -> peniko::Brush {
    peniko::Brush::Solid(to_peniko(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_roundtrip() {
        let c = to_peniko(&LayoutColor::BLACK);
        // AlphaColor<Srgb> stores [r, g, b, a] in components
        assert_eq!(c.components[0], 0.0);
        assert_eq!(c.components[1], 0.0);
        assert_eq!(c.components[2], 0.0);
        assert_eq!(c.components[3], 1.0);
    }

    #[test]
    fn white_roundtrip() {
        let c = to_peniko(&LayoutColor::WHITE);
        assert_eq!(c.components[0], 1.0);
        assert_eq!(c.components[3], 1.0);
    }

    #[test]
    fn transparent_roundtrip() {
        let c = to_peniko(&LayoutColor::TRANSPARENT);
        assert_eq!(c.components[3], 0.0);
    }
}
