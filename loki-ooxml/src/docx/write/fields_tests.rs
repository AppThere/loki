// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the complex-field writer.

use super::*;

/// Renders a field to a UTF-8 string for assertion.
fn render(field: &Field) -> String {
    let mut out = Vec::new();
    let mut w = Writer::new(&mut out);
    write_field(&mut w, field, &RunProps::default());
    String::from_utf8(out).expect("valid UTF-8")
}

#[test]
fn page_field_emits_begin_instr_end() {
    let xml = render(&Field::new(FieldKind::PageNumber));
    assert!(xml.contains(r#"<w:fldChar w:fldCharType="begin"/>"#));
    assert!(xml.contains(r#"<w:instrText xml:space="preserve"> PAGE </w:instrText>"#));
    assert!(xml.contains(r#"<w:fldChar w:fldCharType="end"/>"#));
    // No snapshot ⇒ no `separate` and no result run.
    assert!(!xml.contains(r#"w:fldCharType="separate""#));
}

#[test]
fn field_with_snapshot_emits_separate_and_result() {
    let field = Field::new(FieldKind::PageCount).with_current_value("7");
    let xml = render(&field);
    assert!(xml.contains(r#"<w:instrText xml:space="preserve"> NUMPAGES </w:instrText>"#));
    assert!(xml.contains(r#"<w:fldChar w:fldCharType="separate"/>"#));
    assert!(xml.contains('7'));
}

#[test]
fn instruction_strings_are_inverse_of_parser() {
    assert_eq!(field_instruction(&FieldKind::PageNumber), "PAGE");
    assert_eq!(field_instruction(&FieldKind::PageCount), "NUMPAGES");
    assert_eq!(field_instruction(&FieldKind::Title), "TITLE");
    assert_eq!(field_instruction(&FieldKind::Author), "AUTHOR");
    assert_eq!(field_instruction(&FieldKind::Subject), "SUBJECT");
    assert_eq!(field_instruction(&FieldKind::FileName), "FILENAME");
    assert_eq!(field_instruction(&FieldKind::WordCount), "NUMWORDS");
}

#[test]
fn date_field_includes_format_switch() {
    let kind = FieldKind::Date {
        format: Some("MMMM d, yyyy".to_string()),
    };
    assert_eq!(field_instruction(&kind), r#"DATE \@ "MMMM d, yyyy""#);

    let no_fmt = FieldKind::Date { format: None };
    assert_eq!(field_instruction(&no_fmt), "DATE");
}

#[test]
fn cross_reference_uses_ref_or_pageref() {
    let number = FieldKind::CrossReference {
        target: "_Bm".to_string(),
        format: CrossRefFormat::Number,
    };
    assert_eq!(field_instruction(&number), "REF _Bm");

    let page = FieldKind::CrossReference {
        target: "_Bm".to_string(),
        format: CrossRefFormat::Page,
    };
    assert_eq!(field_instruction(&page), "PAGEREF _Bm");
}

#[test]
fn raw_field_is_verbatim() {
    let kind = FieldKind::Raw {
        instruction: r#"HYPERLINK "https://example.com""#.to_string(),
    };
    assert_eq!(
        field_instruction(&kind),
        r#"HYPERLINK "https://example.com""#
    );
}
