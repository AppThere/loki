// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::style::props::char_props::{CharProps, HighlightColor};
use loki_primitives::units::Points;

use super::{emit_char_props, write_char_props_elem};

/// Renders `cp` through [`emit_char_props`] and returns the UTF-8 XML fragment.
fn emit(cp: &CharProps) -> String {
    let mut buf = Vec::new();
    let mut w = quick_xml::Writer::new(&mut buf);
    emit_char_props(&mut w, cp);
    String::from_utf8(buf).expect("XML is valid UTF-8")
}

/// Renders `cp` through [`write_char_props_elem`] (the styles path, which wraps
/// the children in `<w:rPr>`).
fn emit_elem(cp: &CharProps) -> String {
    let mut buf = Vec::new();
    let mut w = quick_xml::Writer::new(&mut buf);
    write_char_props_elem(&mut w, cp);
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
fn emboss_and_imprint_are_emitted() {
    let cp = CharProps {
        emboss: Some(true),
        imprint: Some(true),
        ..Default::default()
    };
    let xml = emit(&cp);
    assert!(xml.contains("<w:emboss/>"), "xml = {xml}");
    assert!(xml.contains("<w:imprint/>"), "xml = {xml}");
}

#[test]
fn character_border_is_emitted() {
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_primitives::color::DocumentColor;
    let cp = CharProps {
        character_border: Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(1.0),
            color: Some(DocumentColor::from_hex("#C00000").expect("valid hex")),
            spacing: Some(Points::new(1.0)),
        }),
        ..Default::default()
    };
    let xml = emit(&cp);
    assert!(
        xml.contains(r#"<w:bdr w:val="single" w:sz="8" w:space="1" w:color="C00000"/>"#),
        "xml = {xml}"
    );
}

#[test]
fn numeric_weight_at_least_600_collapses_to_bold() {
    // DOCX has no numeric weight; a heavy font_weight with no explicit boolean
    // exports as w:b (the >= 600 => bold rule, at export).
    let heavy = CharProps {
        font_weight: Some(700),
        ..Default::default()
    };
    assert!(emit(&heavy).contains("<w:b/>"), "xml = {}", emit(&heavy));

    let light = CharProps {
        font_weight: Some(400),
        ..Default::default()
    };
    assert!(!emit(&light).contains("<w:b/>"));

    // An explicit bold:Some(false) still suppresses w:b even at a heavy weight.
    let unbolded = CharProps {
        font_weight: Some(700),
        bold: Some(false),
        ..Default::default()
    };
    assert!(!emit(&unbolded).contains("<w:b/>"));
}

#[test]
fn styles_rpr_wraps_a_single_previously_ungated_property() {
    // The styles path (write_char_props_elem) must emit <w:rPr> when the only
    // property is one the old has_content gate omitted — e.g. all-caps or a
    // language tag — or the property was silently dropped on export.
    use loki_doc_model::meta::LanguageTag;
    let caps_only = CharProps {
        all_caps: Some(true),
        ..Default::default()
    };
    let xml = emit_elem(&caps_only);
    assert!(xml.contains("<w:rPr>"), "xml = {xml}");
    assert!(xml.contains("<w:caps/>"), "xml = {xml}");

    let lang_only = CharProps {
        language: Some(LanguageTag::new("fr-FR")),
        ..Default::default()
    };
    let xml = emit_elem(&lang_only);
    assert!(xml.contains("<w:rPr>"), "xml = {xml}");
    assert!(xml.contains("w:lang"), "xml = {xml}");
}

#[test]
fn styles_rpr_is_empty_when_no_property_is_set() {
    assert_eq!(emit_elem(&CharProps::default()), "");
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
