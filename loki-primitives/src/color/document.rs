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

use super::theme::{ThemeColor, ThemeColorSlot};
use appthere_color::{CmykColor, RgbColor};

/// Error when parsing hex color strings.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ColorParseError {
    /// Received invalid hex format.
    #[error("invalid hex color: expected #RRGGBB or #RRGGBBAA, got {input:?}")]
    InvalidFormat {
        /// Erroneous input string value.
        input: String,
    },
}

/// A document-semantic color resolving via a theme or intrinsic value.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DocumentColor {
    /// A direct RGB color value. The encoding is profile-relative; without an
    /// associated ICC profile this is treated as sRGB for display purposes.
    Rgb(RgbColor),

    /// A CMYK color for print output intent.
    /// For display, use `appthere_color::ColorTransform` to convert to RGB
    /// via ICC profiles in `loki-pdf`. Do not convert naively here.
    Cmyk(CmykColor),

    /// A reference to a named theme color slot, with optional tint/shade.
    /// `tint` is in [-1.0, 1.0]: negative = shade (darken towards black),
    /// positive = tint (lighten towards white). Zero = the slot color as-is.
    Theme {
        /// Abstract component naming a palette index.
        slot: ThemeColorSlot,
        /// Floating point scalar adjusting lightness.
        tint: f32,
    },

    /// Fully transparent — no color.
    Transparent,
}

impl DocumentColor {
    /// Attempt to resolve this color to a concrete `RgbColor` given a theme.
    ///
    /// Returns `None` if the color is `Transparent`.
    /// Returns `None` if the color is `Cmyk` — CMYK requires an ICC transform
    /// via `appthere_color::ColorTransform`, which is not available here.
    /// Callers requiring CMYK display conversion should use `loki-pdf` or
    /// call `appthere_color::ColorTransform` directly with the relevant profiles.
    #[must_use]
    pub fn resolve_rgb(&self, theme: &ThemeColor) -> Option<RgbColor> {
        match self {
            Self::Rgb(rgb) => Some(*rgb),
            Self::Cmyk(_) => None,
            Self::Theme { slot, tint } => {
                let base = theme.get(*slot)?;
                Some(ThemeColor::apply_tint(*base, *tint))
            }
            Self::Transparent => None,
        }
    }

    /// Returns `true` if this color references a theme slot.
    #[must_use]
    pub fn is_theme_color(&self) -> bool {
        matches!(self, Self::Theme { .. })
    }

    /// Parse a hex color string (`#RRGGBB` or `#RRGGBBAA`) into a
    /// `DocumentColor::Rgb`. The alpha channel from `#RRGGBBAA` is silently
    /// discarded — `DocumentColor::Rgb` does not carry alpha.
    pub fn from_hex(s: &str) -> Result<Self, ColorParseError> {
        let err = || ColorParseError::InvalidFormat { input: s.to_string() };

        if !s.starts_with('#') {
            return Err(err());
        }
        let hex = &s[1..];
        if hex.len() != 6 && hex.len() != 8 {
            return Err(err());
        }

        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| err())?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| err())?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| err())?;

        Ok(Self::Rgb(RgbColor::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
        )))
    }

    /// Serialise a `DocumentColor::Rgb` variant to a `#RRGGBB` hex string.
    /// Returns `None` for non-Rgb variants.
    #[must_use]
    pub fn to_hex(&self) -> Option<String> {
        if let Self::Rgb(rgb) = self {
            let r = (rgb.red() * 255.0).round() as u8;
            let g = (rgb.green() * 255.0).round() as u8;
            let b = (rgb.blue() * 255.0).round() as u8;
            Some(format!("#{r:02X}{g:02X}{b:02X}"))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use appthere_color::{CmykColor, RgbColor};
    use std::collections::HashMap;

    #[test]
    fn test_hex_parsing() {
        assert_eq!(
            DocumentColor::from_hex("#FF8000").unwrap(),
            DocumentColor::Rgb(RgbColor::new(1.0, 128.0 / 255.0, 0.0))
        );
        assert_eq!(
            DocumentColor::from_hex("#FF8000CC").unwrap(),
            DocumentColor::Rgb(RgbColor::new(1.0, 128.0 / 255.0, 0.0))
        );

        assert!(matches!(
            DocumentColor::from_hex("not-a-color"),
            Err(ColorParseError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn test_hex_roundtrip() {
        let c = DocumentColor::from_hex("#123456").unwrap();
        assert_eq!(c.to_hex().unwrap(), "#123456");
    }

    #[test]
    fn test_resolve() {
        let mut map = HashMap::new();
        map.insert(ThemeColorSlot::Accent1, RgbColor::new(1.0, 0.0, 0.0));
        let theme = ThemeColor::new(map);

        assert!(DocumentColor::Transparent.resolve_rgb(&theme).is_none());

        let rgb = RgbColor::new(0.0, 1.0, 0.0);
        assert_eq!(
            DocumentColor::Rgb(rgb).resolve_rgb(&theme).unwrap(),
            rgb
        );

        assert!(DocumentColor::Cmyk(CmykColor::new(1.0, 1.0, 1.0, 1.0))
            .resolve_rgb(&theme)
            .is_none());

        let t = theme.get(ThemeColorSlot::Accent1).unwrap();
        assert_eq!(
            DocumentColor::Theme {
                slot: ThemeColorSlot::Accent1,
                tint: 0.0
            }
            .resolve_rgb(&theme)
            .unwrap()
            .to_array(),
            t.to_array()
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde() {
        let color = DocumentColor::Theme {
            slot: ThemeColorSlot::Dark1,
            tint: 0.25,
        };
        let js = serde_json::to_string(&color).unwrap();
        let parsed: DocumentColor = serde_json::from_str(&js).unwrap();
        assert_eq!(color, parsed);
    }
}
