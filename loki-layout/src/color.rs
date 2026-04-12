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

//! Layout-internal RGBA color type.
//!
//! [`LayoutColor`] is intentionally decoupled from `appthere_color` so that
//! the layout output types have no dependency on the color-management stack.
//! A [`From`] impl bridges `appthere_color::RgbColor` → [`LayoutColor`].

/// RGBA color for layout output. Components in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutColor {
    /// Red component.
    pub r: f32,
    /// Green component.
    pub g: f32,
    /// Blue component.
    pub b: f32,
    /// Alpha component (0.0 = fully transparent, 1.0 = fully opaque).
    pub a: f32,
}

impl LayoutColor {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    /// Opaque black.
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

    /// Opaque white.
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    /// Creates a new color from its components.
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Returns a copy of this color with the alpha component replaced.
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
}

/// Convert from `appthere_color::RgbColor` (sRGB, gamma-encoded).
///
/// The alpha is set to `1.0` (fully opaque) since `RgbColor` carries no
/// alpha channel.
impl From<appthere_color::RgbColor> for LayoutColor {
    fn from(c: appthere_color::RgbColor) -> Self {
        Self::new(c.red(), c.green(), c.blue(), 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_alpha_replaces_alpha() {
        let c = LayoutColor::new(0.5, 0.3, 0.1, 1.0);
        let c2 = c.with_alpha(0.5);
        assert_eq!(c2.r, 0.5);
        assert_eq!(c2.g, 0.3);
        assert_eq!(c2.b, 0.1);
        assert_eq!(c2.a, 0.5);
    }

    #[test]
    fn constants() {
        assert_eq!(LayoutColor::TRANSPARENT.a, 0.0);
        assert_eq!(LayoutColor::BLACK.r, 0.0);
        assert_eq!(LayoutColor::BLACK.a, 1.0);
        assert_eq!(LayoutColor::WHITE.r, 1.0);
        assert_eq!(LayoutColor::WHITE.a, 1.0);
    }

    #[test]
    fn from_rgb_color() {
        let rgb = appthere_color::RgbColor::new(0.2, 0.4, 0.6);
        let lc = LayoutColor::from(rgb);
        assert_eq!(lc.r, 0.2);
        assert_eq!(lc.g, 0.4);
        assert_eq!(lc.b, 0.6);
        assert_eq!(lc.a, 1.0);
    }
}
