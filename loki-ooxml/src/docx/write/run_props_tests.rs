// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::style::props::char_props::{CharProps, HighlightColor};
use loki_primitives::units::Points;

use super::emit_char_props;

/// Renders `cp` through [`emit_char_props`] and returns the UTF-8 XML fragment.
fn emit(cp: &CharProps) -> String {
    let mut buf = Vec::new();
    let mut w = quick_xml::Writer::new(&mut buf);
    emit_char_props(&mut w, cp);
    String::from_utf8(buf).expect("XML is valid UTF-8")
}

#[test]
fn highlight_is_emitted() {
    let cp = CharProps {
        highlight_color: Some(HighlightColor::Yellow),
        ..Default::default()
    };
    assert!(emit(&cp).contains(r#"<w:highlight w:val="yellow"/>"#));
}

#[test]
fn highlight_none_emits_nothing() {
    let cp = CharProps {
        highlight_color: Some(HighlightColor::None),
        ..Default::default()
    };
    assert!(!emit(&cp).contains("w:highlight"));
}

#[test]
fn letter_spacing_is_emitted_in_twips() {
    // 2 pt → 40 twips (the reference fixture's `w:spacing w:val="40"`).
    let cp = CharProps {
        letter_spacing: Some(Points::new(2.0)),
        ..Default::default()
    };
    assert!(emit(&cp).contains(r#"<w:spacing w:val="40"/>"#));
}

#[test]
fn all_caps_and_shadow_are_emitted() {
    let cp = CharProps {
        all_caps: Some(true),
        shadow: Some(true),
        ..Default::default()
    };
    let xml = emit(&cp);
    assert!(xml.contains("<w:caps/>"), "xml = {xml}");
    assert!(xml.contains("<w:shadow/>"), "xml = {xml}");
}

#[test]
fn scale_emits_integer_percent() {
    let cp = CharProps {
        scale: Some(1.5),
        ..Default::default()
    };
    assert!(emit(&cp).contains(r#"<w:w w:val="150"/>"#));
}

#[test]
fn kerning_emits_threshold_and_disabled_zero() {
    let on = CharProps {
        kerning: Some(true),
        ..Default::default()
    };
    assert!(emit(&on).contains(r#"<w:kern w:val="2"/>"#));
    let off = CharProps {
        kerning: Some(false),
        ..Default::default()
    };
    assert!(emit(&off).contains(r#"<w:kern w:val="0"/>"#));
}

#[test]
fn complex_font_and_size_are_emitted() {
    let cp = CharProps {
        font_name_complex: Some("Arabic Typesetting".to_string()),
        font_size_complex: Some(Points::new(14.0)),
        ..Default::default()
    };
    let xml = emit(&cp);
    assert!(xml.contains(r#"w:cs="Arabic Typesetting""#), "xml = {xml}");
    assert!(xml.contains(r#"<w:szCs w:val="28"/>"#), "xml = {xml}");
}
