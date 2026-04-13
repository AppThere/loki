// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
