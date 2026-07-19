// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `document_run` (extracted for the 300-line file ceiling).

use super::*;
use crate::docx::model::paragraph::DocxRunChild;

/// Concatenate the text of a `<w:r>` fragment's `Text` children.
fn run_text(xml: &str) -> String {
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if local_name(e.local_name().as_ref()) == b"r" => break,
            Event::Eof => panic!("no w:r"),
            _ => {}
        }
    }
    parse_run(&mut reader)
        .unwrap()
        .children
        .iter()
        .filter_map(|c| match c {
            DocxRunChild::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn no_break_hyphen_becomes_u2011() {
    let xml = r#"<w:r xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:t>non</w:t><w:noBreakHyphen/><w:t>breaking</w:t></w:r>"#;
    assert_eq!(run_text(xml), "non\u{2011}breaking");
}

#[test]
fn soft_hyphen_becomes_u00ad() {
    let xml = r#"<w:r xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:t>optional</w:t><w:softHyphen/><w:t>hyphen</w:t></w:r>"#;
    assert_eq!(run_text(xml), "optional\u{00ad}hyphen");
}

#[test]
fn parses_emboss_imprint_shadow() {
    let xml = r#"<w:r xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:rPr><w:emboss/><w:imprint/><w:shadow/></w:rPr><w:t>x</w:t></w:r>"#;
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if local_name(e.local_name().as_ref()) == b"r" => break,
            Event::Eof => panic!("no w:r"),
            _ => {}
        }
    }
    let rpr = parse_run(&mut reader).unwrap().rpr.expect("rpr");
    assert_eq!(rpr.emboss, Some(true), "emboss");
    assert_eq!(rpr.imprint, Some(true), "imprint");
    assert_eq!(rpr.shadow, Some(true), "shadow");
}
