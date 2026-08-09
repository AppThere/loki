// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for parsing the advisory page-style part (Spec 08 T6.5, D-02).

use super::parse_page_style_part;
use crate::docx::page_style_part::PageStyleMap;

fn parse(xml: &str) -> Option<PageStyleMap> {
    parse_page_style_part(xml.as_bytes())
}

const NS: &str = r#"xmlns="http://appthere.dev/loki/2026/pageStyles""#;

#[test]
fn a_well_formed_part_parses() {
    let map = parse(&format!(
        r#"<pageStyles {NS} sectionCount="2">
             <style id="Body" displayName="Body Text"/>
             <style id="Cover"/>
             <section index="0" style="Body"/>
             <section index="1" style="Cover"/>
           </pageStyles>"#
    ))
    .expect("parses");
    assert_eq!(map.styles.len(), 2);
    assert_eq!(map.styles[0].display_name.as_deref(), Some("Body Text"));
    assert_eq!(map.styles[1].display_name, None);
    assert_eq!(
        map.sections,
        vec![Some("Body".to_string()), Some("Cover".to_string())]
    );
}

/// The declared `sectionCount` is authoritative, not the number of `<section>`
/// elements. A part claiming four sections but listing two yields a four-long
/// map with holes — which then fails the count check against a real document,
/// rather than quietly passing as a two-section map.
#[test]
fn the_declared_count_governs_the_map_length() {
    let map = parse(&format!(
        r#"<pageStyles {NS} sectionCount="4">
             <style id="Body"/>
             <section index="0" style="Body"/>
             <section index="2" style="Body"/>
           </pageStyles>"#
    ))
    .expect("parses");
    assert_eq!(map.sections.len(), 4);
    assert_eq!(map.sections[1], None);
    assert_eq!(map.sections[3], None);
}

/// Every malformed shape, each asserted `None`. A parser exercised only on what
/// it accepts reports nothing about what it lets through — and this one's whole
/// job is refusing input it should not trust.
#[test]
fn malformed_parts_are_rejected() {
    for (xml, why) in [
        (
            format!(r#"<pageStyles {NS}><style id="Body"/></pageStyles>"#),
            "no sectionCount",
        ),
        (
            format!(r#"<pageStyles {NS} sectionCount="nope"/>"#),
            "non-numeric sectionCount",
        ),
        (
            format!(r#"<pageStyles {NS} sectionCount="-1"/>"#),
            "negative sectionCount",
        ),
        (
            format!(
                r#"<pageStyles {NS} sectionCount="1">
                     <style id="Body"/><section index="5" style="Body"/>
                   </pageStyles>"#
            ),
            "index beyond the declared count",
        ),
        (
            format!(
                r#"<pageStyles {NS} sectionCount="2">
                     <style id="Body"/>
                     <section index="0" style="Body"/>
                     <section index="0" style="Body"/>
                   </pageStyles>"#
            ),
            "two entries for one section",
        ),
        (
            format!(r#"<pageStyles {NS} sectionCount="1"><style/></pageStyles>"#),
            "style with no id",
        ),
        (
            format!(r#"<pageStyles {NS} sectionCount="1"><style id=""/></pageStyles>"#),
            "style with an empty id",
        ),
        (
            format!(r#"<pageStyles {NS} sectionCount="1"><section index="0"/></pageStyles>"#),
            "section with no style",
        ),
        (
            format!(r#"<pageStyles {NS} sectionCount="1"><style id="A"></pageStyles>"#),
            "unclosed element",
        ),
        (
            format!(r#"<pageStyles {NS} sectionCount="99999999"/>"#),
            "an allocation-sized count",
        ),
        (String::from("not xml at all"), "not xml"),
        (String::new(), "empty"),
    ] {
        assert!(parse(&xml).is_none(), "accepted {why}: {xml}");
    }
}

/// Escaped names survive the round trip — the writer escapes, so the reader
/// must unescape, and a name with an ampersand is the case that proves it.
#[test]
fn escaped_names_round_trip() {
    let map = parse(&format!(
        r#"<pageStyles {NS} sectionCount="1">
             <style id="A &amp; B" displayName="the &quot;real&quot; name"/>
             <section index="0" style="A &amp; B"/>
           </pageStyles>"#
    ))
    .expect("parses");
    assert_eq!(map.styles[0].id, "A & B");
    assert_eq!(
        map.styles[0].display_name.as_deref(),
        Some(r#"the "real" name"#)
    );
    assert_eq!(map.sections[0].as_deref(), Some("A & B"));
}

/// A zero-section part is well-formed and describes an empty document; it
/// simply will not match any real one.
#[test]
fn a_zero_section_part_parses_as_empty() {
    let map = parse(&format!(r#"<pageStyles {NS} sectionCount="0"/>"#)).expect("parses");
    assert!(map.sections.is_empty());
}
