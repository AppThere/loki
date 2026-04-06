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

use appthere_color::{LabColor, RgbColor};
use std::collections::HashMap;

/// Named color slots in a document's theme palette.
///
/// Matches the 12-slot model used by both ODF and OOXML themes, ensuring
/// that theme colors can be round-tripped through both formats without loss.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ThemeColorSlot {
    /// Standard Dark 1 variation.
    Dark1,
    /// Standard Dark 2 variation.
    Dark2,
    /// Standard Light 1 variation.
    Light1,
    /// Standard Light 2 variation.
    Light2,
    /// Primary accent tone 1.
    Accent1,
    /// Decorating accent tone 2.
    Accent2,
    /// Decorating accent tone 3.
    Accent3,
    /// Decorating accent tone 4.
    Accent4,
    /// Decorating accent tone 5.
    Accent5,
    /// Decorating accent tone 6.
    Accent6,
    /// General hyperlink foreground indicator.
    Hyperlink,
    /// Visited hyperlink foreground indicator.
    FollowedHyperlink,
}

/// A complete document color theme mapping each slot to a concrete RGB color.
///
/// Colors are stored as `appthere_color::RgbColor` values. For display
/// purposes these are treated as sRGB-encoded; for ICC-accurate output use
/// `appthere_color::ColorTransform` with the document's embedded profile.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ThemeColor {
    slots: HashMap<ThemeColorSlot, RgbColor>,
}

impl ThemeColor {
    /// Construct a new theme from mappings.
    #[must_use]
    pub fn new(slots: HashMap<ThemeColorSlot, RgbColor>) -> Self {
        Self { slots }
    }

    /// Retrieve a resolved color from this theme map, conditionally mapping it natively.
    #[must_use]
    pub fn get(&self, slot: ThemeColorSlot) -> Option<&RgbColor> {
        self.slots.get(&slot)
    }

    /// Apply tint (positive) or shade (negative) to an `RgbColor`.
    ///
    /// # Implementation note
    ///
    /// The interpolation is performed in CIE L*a*b* space using
    /// `appthere_color::LabColor` for perceptual uniformity. The input
    /// `RgbColor` is treated as sRGB-encoded for this conversion. For
    /// ICC-accurate results, convert via `appthere_color::ColorTransform`
    /// before calling this method.
    ///
    /// `amount` is clamped to `[-1.0, 1.0]`. At `1.0` the result is white;
    /// at `-1.0` the result is black; at `0.0` the input is returned unchanged.
    #[must_use]
    pub fn apply_tint(color: RgbColor, amount: f32) -> RgbColor {
        let amount = amount.clamp(-1.0, 1.0);
        if amount == 0.0 {
            return color;
        }

        let lab = rgb_to_lab(color);
        let current_l = lab.l();
        let current_a = lab.a();
        let current_b = lab.b();

        let l: f32;
        let a_factor: f32;

        if amount > 0.0 {
            // Tint: interpolate L toward 100
            l = current_l + (100.0 - current_l) * amount;
            // Scale a,b to 0 gradually
            a_factor = (100.0 - l) / (100.0 - current_l).max(1e-6);
        } else {
            // Shade: interpolate L toward 0
            let factor = -amount;
            l = current_l - current_l * factor;
            // Scale a,b to 0 gradually
            a_factor = l / current_l.max(1e-6);
        }

        let resultant = LabColor::new(l, current_a * a_factor, current_b * a_factor);
        lab_to_rgb(resultant)
    }
}

// IEC 61966-2-1 standard approximations for quick linear/non-linear math.
fn uncomp_srgb(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn comp_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn lab_f(t: f64) -> f64 {
    if t > (6.0 / 29.0_f64).powi(3) {
        t.cbrt()
    } else {
        (1.0 / 3.0) * (29.0 / 6.0_f64).powi(2) * t + 4.0 / 29.0
    }
}

fn inv_lab_f(t: f64) -> f64 {
    if t > 6.0 / 29.0 {
        t.powi(3)
    } else {
        3.0 * (6.0 / 29.0_f64).powi(2) * (t - 4.0 / 29.0)
    }
}

fn rgb_to_lab(rgb: RgbColor) -> LabColor {
    let r = uncomp_srgb(rgb.red()) as f64;
    let g = uncomp_srgb(rgb.green()) as f64;
    let b = uncomp_srgb(rgb.blue()) as f64;

    let x = (r * 0.4124 + g * 0.3576 + b * 0.1805) / 0.95047;
    let y = (r * 0.2126 + g * 0.7152 + b * 0.0722) / 1.00000;
    let z = (r * 0.0193 + g * 0.1192 + b * 0.9505) / 1.08883;

    let fx = lab_f(x);
    let fy = lab_f(y);
    let fz = lab_f(z);

    LabColor::new(
        (116.0 * fy - 16.0) as f32,
        (500.0 * (fx - fy)) as f32,
        (200.0 * (fy - fz)) as f32,
    )
}

fn lab_to_rgb(lab: LabColor) -> RgbColor {
    let l = lab.l() as f64;
    let a = lab.a() as f64;
    let b = lab.b() as f64;

    let fy = (l + 16.0) / 116.0;
    let fx = (a / 500.0) + fy;
    let fz = fy - (b / 200.0);

    let x = inv_lab_f(fx) * 0.95047;
    let y = inv_lab_f(fy) * 1.00000;
    let z = inv_lab_f(fz) * 1.08883;

    let r = x * 3.2406 + y * -1.5372 + z * -0.4986;
    let g = x * -0.9689 + y * 1.8758 + z * 0.0415;
    let b_comp = x * 0.0557 + y * -0.2040 + z * 1.0570;

    RgbColor::new(
        comp_srgb(r as f32),
        comp_srgb(g as f32),
        comp_srgb(b_comp as f32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_apply_tint() {
        let c = RgbColor::new(0.5, 0.0, 0.0);
        let normal = ThemeColor::apply_tint(c, 0.0);
        assert_relative_eq!(normal.red(), 0.5, epsilon = 1e-4);
        assert_relative_eq!(normal.green(), 0.0, epsilon = 1e-4);
        assert_relative_eq!(normal.blue(), 0.0, epsilon = 1e-4);

        let white = ThemeColor::apply_tint(c, 1.0);
        assert_relative_eq!(white.red(), 1.0, epsilon = 1e-3);
        assert_relative_eq!(white.green(), 1.0, epsilon = 1e-3);
        assert_relative_eq!(white.blue(), 1.0, epsilon = 1e-3);

        let black = ThemeColor::apply_tint(c, -1.0);
        assert_relative_eq!(black.red(), 0.0, epsilon = 1e-3);
        assert_relative_eq!(black.green(), 0.0, epsilon = 1e-3);
        assert_relative_eq!(black.blue(), 0.0, epsilon = 1e-3);

        let c_lab = rgb_to_lab(c);
        let tinted = ThemeColor::apply_tint(c, 0.5);
        let tinted_lab = rgb_to_lab(tinted);
        assert!(tinted_lab.l() > c_lab.l());

        let shaded = ThemeColor::apply_tint(c, -0.5);
        let shaded_lab = rgb_to_lab(shaded);
        assert!(shaded_lab.l() < c_lab.l());
    }

    #[test]
    fn test_theme_get() {
        let mut map = HashMap::new();
        map.insert(ThemeColorSlot::Accent1, RgbColor::new(1.0, 1.0, 1.0));
        let t = ThemeColor::new(map);
        assert!(t.get(ThemeColorSlot::Accent1).is_some());
        assert!(t.get(ThemeColorSlot::Dark1).is_none());
    }
}
