// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

// Casts are of u8-range hex components (0..=255), exactly representable in f32.
#![allow(clippy::cast_precision_loss)]

use super::*;

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
fn shading_texture_pattern_blends_color_over_fill() {
    // A line/cross texture is flattened to a tint of the foreground over fill.
    // diagStripe ≈ 50 % of orange (ED7D31) over white (FFFFFF).
    let c = resolve_shading(Some("FFFFFF"), Some("diagStripe"), Some("ED7D31")).unwrap();
    let expect = |fg: f32| 1.0 * 0.5 + fg * 0.5;
    assert!((c.red() - expect(0xED as f32 / 255.0)).abs() < 1e-4);
    assert!((c.green() - expect(0x7D as f32 / 255.0)).abs() < 1e-4);
    // Lighter than the solid foreground (a visible-but-tinted stripe).
    assert!(c.blue() > 0x31 as f32 / 255.0);
}

#[test]
fn shading_thin_texture_is_lighter_than_bold() {
    let thin = resolve_shading(Some("FFFFFF"), Some("thinDiagStripe"), Some("000000")).unwrap();
    let bold = resolve_shading(Some("FFFFFF"), Some("diagStripe"), Some("000000")).unwrap();
    // Less ink → closer to white → higher channel value.
    assert!(thin.red() > bold.red());
}

#[test]
fn shading_unknown_value_falls_back_to_fill() {
    let c = resolve_shading(Some("97BC62"), Some("someFutureVal"), Some("000000")).unwrap();
    assert!((c.green() - 0xBC as f32 / 255.0).abs() < 1e-4);
}

// ── XXE posture (audit-2026-06 S-5) ──────────────────────────────────────────

/// A DOCTYPE-declared external entity must never be fetched or expanded.
/// quick-xml surfaces `&xxe;` as a `GeneralRef`, and `resolve_general_ref`
/// resolves only the five predefined XML entities — everything else stays a
/// literal `&name;`. This test fails if entity/DTD expansion is ever enabled.
#[test]
fn external_entities_are_never_resolved() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE w:document [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>&xxe;</w:t></w:r></w:p></w:body>
</w:document>"#;
    let doc = crate::docx::reader::document::parse_document(xml).expect("document parses");
    let text = format!("{doc:?}");
    assert!(
        !text.contains("root:"),
        "external entity content must never appear in the parsed document"
    );
    assert!(
        text.contains("&xxe;"),
        "an unresolvable entity reference stays literal, got: {text}"
    );
}
