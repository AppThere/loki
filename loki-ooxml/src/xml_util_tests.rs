// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;

// ── bool_attr ─────────────────────────────────────────────────────────────

#[test]
fn bool_attr_true_values() {
    assert!(bool_attr("1"));
    assert!(bool_attr("true"));
    assert!(bool_attr("on"));
    // Unknown values also default to true per ECMA-376 §17.7.3
    assert!(bool_attr("yes"));
    assert!(bool_attr(""));
}

#[test]
fn bool_attr_false_values() {
    assert!(!bool_attr("0"));
    assert!(!bool_attr("false"));
    assert!(!bool_attr("off"));
}

// ── twips_to_points ───────────────────────────────────────────────────────

#[test]
fn twips_to_points_zero() {
    assert_eq!(twips_to_points(0).value(), 0.0);
}

#[test]
fn twips_to_points_one_pt() {
    assert_eq!(twips_to_points(20).value(), 1.0);
}

#[test]
fn twips_to_points_720() {
    // 720 twips = 36 pt
    assert_eq!(twips_to_points(720).value(), 36.0);
}

#[test]
fn twips_to_points_negative() {
    assert_eq!(twips_to_points(-20).value(), -1.0);
}

// ── half_points_to_points ─────────────────────────────────────────────────

#[test]
fn half_points_to_points_24() {
    // 24 half-points = 12 pt
    assert_eq!(half_points_to_points(24).value(), 12.0);
}

#[test]
fn half_points_to_points_zero() {
    assert_eq!(half_points_to_points(0).value(), 0.0);
}

// ── emu_to_points ─────────────────────────────────────────────────────────

#[test]
fn emu_to_points_one_pt() {
    assert_eq!(emu_to_points(12700).value(), 1.0);
}

#[test]
fn emu_to_points_one_inch() {
    // 914400 EMU = 72 pt
    assert!((emu_to_points(914_400).value() - 72.0).abs() < f64::EPSILON);
}

#[test]
fn emu_to_points_zero() {
    assert_eq!(emu_to_points(0).value(), 0.0);
}

// ── hex_color ─────────────────────────────────────────────────────────────

#[test]
fn hex_color_red() {
    let c = hex_color("FF0000").unwrap();
    assert!((c.red() - 1.0_f32).abs() < 1e-6);
    assert!(c.green() < 1e-6);
    assert!(c.blue() < 1e-6);
}

#[test]
fn hex_color_black() {
    let c = hex_color("000000").unwrap();
    assert!(c.red() < 1e-6);
    assert!(c.green() < 1e-6);
    assert!(c.blue() < 1e-6);
}

#[test]
fn hex_color_white() {
    let c = hex_color("FFFFFF").unwrap();
    assert!((c.red() - 1.0_f32).abs() < 1e-6);
    assert!((c.green() - 1.0_f32).abs() < 1e-6);
    assert!((c.blue() - 1.0_f32).abs() < 1e-6);
}

#[test]
fn hex_color_auto_is_none() {
    assert!(hex_color("auto").is_none());
}

#[test]
fn hex_color_empty_is_none() {
    assert!(hex_color("").is_none());
}

#[test]
fn hex_color_invalid_chars_is_none() {
    assert!(hex_color("GGGGGG").is_none());
}

#[test]
fn hex_color_wrong_length_is_none() {
    assert!(hex_color("FFF").is_none());
    assert!(hex_color("FFFFFFF").is_none());
}

#[test]
fn hex_color_lowercase() {
    // OOXML emits uppercase but parsers should tolerate lowercase
    let c = hex_color("ff8000").unwrap();
    assert!((c.red() - 1.0_f32).abs() < 1e-6);
    assert!((c.green() - 128.0_f32 / 255.0).abs() < 1e-4);
    assert!(c.blue() < 1e-6);
}

// ── resolve_shading ───────────────────────────────────────────────────────

#[test]
fn shading_clear_uses_fill() {
    let c = resolve_shading(Some("CADCFC"), Some("clear"), None).unwrap();
    assert!((c.red() - 0xCA as f32 / 255.0).abs() < 1e-4);
    assert!((c.blue() - 0xFC as f32 / 255.0).abs() < 1e-4);
}

#[test]
fn shading_clear_auto_fill_is_none() {
    // The common no-op shading `<w:shd val="clear" fill="auto"/>`.
    assert!(resolve_shading(Some("auto"), Some("clear"), Some("auto")).is_none());
}

#[test]
fn shading_solid_uses_color() {
    let c = resolve_shading(Some("FFFFFF"), Some("solid"), Some("1C7293")).unwrap();
    assert!((c.red() - 0x1C as f32 / 255.0).abs() < 1e-4);
    assert!((c.green() - 0x72 as f32 / 255.0).abs() < 1e-4);
}

#[test]
fn shading_pct25_blends_color_over_fill() {
    // 25 % of teal (1C7293) over white (FFFFFF) → light teal.
    let c = resolve_shading(Some("FFFFFF"), Some("pct25"), Some("1C7293")).unwrap();
    let expect = |fg: f32| 1.0 * 0.75 + fg * 0.25;
    assert!((c.red() - expect(0x1C as f32 / 255.0)).abs() < 1e-4);
    assert!((c.green() - expect(0x72 as f32 / 255.0)).abs() < 1e-4);
    assert!((c.blue() - expect(0x93 as f32 / 255.0)).abs() < 1e-4);
    // The result must be lighter than the solid foreground (visible shade).
    assert!(c.red() > 0x1C as f32 / 255.0);
}

#[test]
fn shading_nil_is_none() {
    assert!(resolve_shading(Some("CADCFC"), Some("nil"), None).is_none());
}

#[test]
fn shading_unknown_texture_falls_back_to_fill() {
    let c = resolve_shading(Some("97BC62"), Some("horzStripe"), Some("000000")).unwrap();
    assert!((c.green() - 0xBC as f32 / 255.0).abs() < 1e-4);
}
