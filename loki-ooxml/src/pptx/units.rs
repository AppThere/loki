// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `DrawingML` unit conversions and small value mappings shared by the PPTX
//! shape/text parsers.

use loki_graphics::PresetShape;
use loki_primitives::color::DocumentColor;

/// English Metric Units per point (ECMA-376 §20.1.2.1: 914 400 EMU/inch).
const EMU_PER_PT: f64 = 12700.0;

/// Sanity ceiling on any single EMU length read from a file: 100 metres
/// (≈ 3.6e9 EMU). Slide and shape geometry beyond this is not a plausible
/// document — it is malformed or adversarial input that would otherwise flow
/// unbounded into layout arithmetic (audit-2026-06 S-2).
const MAX_EMU: i64 = 3_600_000_000;

/// Converts an EMU length to points, clamped to ±[`MAX_EMU`].
#[allow(clippy::cast_precision_loss)] // document measurements; precision loss is fine
pub(super) fn emu_to_pt(emu: i64) -> f64 {
    emu.clamp(-MAX_EMU, MAX_EMU) as f64 / EMU_PER_PT
}

/// Converts a `DrawingML` rotation (60 000ths of a degree, clockwise) to degrees.
#[allow(clippy::cast_precision_loss)]
pub(super) fn rot_to_deg(rot: i64) -> f64 {
    rot as f64 / 60_000.0
}

/// Converts a run font size (`a:rPr@sz`, hundredths of a point) to points.
#[allow(clippy::cast_precision_loss)]
pub(super) fn font_size_to_pt(sz: i64) -> f64 {
    sz as f64 / 100.0
}

/// Parses an OOXML truthy attribute (`"1"`, `"true"`, `"on"` → true).
pub(super) fn parse_bool(val: &str) -> bool {
    matches!(val, "1" | "true" | "on")
}

/// Maps a `DrawingML` preset-geometry name (`a:prstGeom@prst`) to a
/// [`PresetShape`]. Unknown presets fall back to a rectangle.
pub(super) fn preset_from_prst(prst: &str) -> PresetShape {
    match prst {
        "ellipse" => PresetShape::Ellipse,
        "roundRect" => PresetShape::RoundedRectangle { corner_radius: 0.0 },
        "line" | "straightConnector1" => PresetShape::Line,
        "triangle" => PresetShape::Triangle,
        "rtTriangle" => PresetShape::RightTriangle,
        "diamond" => PresetShape::Diamond,
        "pentagon" => PresetShape::Pentagon,
        "hexagon" => PresetShape::Hexagon,
        // "rect", "snip*Rect", and anything unrecognised render as a rectangle.
        _ => PresetShape::Rectangle,
    }
}

/// Builds a [`DocumentColor`] from a 6-hex-digit `a:srgbClr@val` (no `#`).
pub(super) fn color_from_srgb(hex: &str) -> Option<DocumentColor> {
    DocumentColor::from_hex(&format!("#{hex}")).ok()
}

// ── Inverse conversions (model → `DrawingML`), used by the exporter ─────────────

/// Converts points to EMU.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub(super) fn pt_to_emu(pt: f64) -> i64 {
    (pt * EMU_PER_PT).round() as i64
}

/// Converts degrees to a `DrawingML` rotation (60 000ths of a degree).
#[allow(clippy::cast_possible_truncation)]
pub(super) fn deg_to_rot(deg: f64) -> i64 {
    (deg * 60_000.0).round() as i64
}

/// Converts a point font size to `a:rPr@sz` (hundredths of a point).
#[allow(clippy::cast_possible_truncation)]
pub(super) fn pt_to_font_size(pt: f64) -> i64 {
    (pt * 100.0).round() as i64
}

/// The `a:prstGeom@prst` name for a [`PresetShape`].
pub(super) fn prst_from_preset(preset: PresetShape) -> &'static str {
    match preset {
        PresetShape::Rectangle => "rect",
        PresetShape::RoundedRectangle { .. } => "roundRect",
        PresetShape::Ellipse => "ellipse",
        PresetShape::Line => "line",
        PresetShape::Triangle => "triangle",
        PresetShape::RightTriangle => "rtTriangle",
        PresetShape::Diamond => "diamond",
        PresetShape::Pentagon => "pentagon",
        PresetShape::Hexagon => "hexagon",
    }
}

/// The 6-hex-digit `a:srgbClr@val` (no `#`) for a color, or `None` for
/// non-RGB (e.g. theme) colors that can't be flattened.
pub(super) fn srgb_from_color(color: &DocumentColor) -> Option<String> {
    color
        .to_hex()
        .map(|h| h.trim_start_matches('#').to_string())
}

/// Opaque black, used as the default stroke color when a line has no explicit
/// fill color.
pub(super) fn default_black() -> DocumentColor {
    DocumentColor::Rgb(appthere_color::RgbColor::new(0.0, 0.0, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emu_and_rotation_conversions() {
        assert!((emu_to_pt(914_400) - 72.0).abs() < 1e-9);
        assert!((emu_to_pt(12_700) - 1.0).abs() < 1e-9);
        assert!((rot_to_deg(5_400_000) - 90.0).abs() < 1e-9);
        assert!((font_size_to_pt(1800) - 18.0).abs() < 1e-9);
    }

    #[test]
    fn absurd_emu_lengths_clamp_instead_of_flowing_into_layout() {
        // audit-2026-06 S-2: cx="9223372036854775807" must not produce a
        // near-infinite dimension.
        assert!(emu_to_pt(i64::MAX).is_finite());
        assert!((emu_to_pt(i64::MAX) - emu_to_pt(MAX_EMU)).abs() < 1e-9);
        assert!((emu_to_pt(i64::MIN) - emu_to_pt(-MAX_EMU)).abs() < 1e-9);
        // In-range values are untouched.
        assert!((emu_to_pt(914_400) - 72.0).abs() < 1e-9);
    }

    #[test]
    fn preset_mapping_and_fallback() {
        assert_eq!(preset_from_prst("ellipse"), PresetShape::Ellipse);
        assert_eq!(preset_from_prst("rect"), PresetShape::Rectangle);
        assert_eq!(preset_from_prst("somethingNew"), PresetShape::Rectangle);
    }

    #[test]
    fn bool_and_color() {
        assert!(parse_bool("1"));
        assert!(!parse_bool("0"));
        assert!(color_from_srgb("FF0000").is_some());
        assert!(color_from_srgb("zzzzzz").is_none());
    }
}
