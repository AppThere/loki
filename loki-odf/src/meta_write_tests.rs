// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the shared `meta.xml` renderer.

use super::{MetaFields, escape, meta_xml_from};

#[test]
fn an_empty_field_set_still_renders_a_well_formed_empty_meta() {
    let xml = meta_xml_from(&MetaFields::default(), "1.3");
    assert!(xml.contains("office:version=\"1.3\""));
    assert!(xml.contains("<office:meta></office:meta>"));
    // No stray elements for absent fields.
    assert!(!xml.contains("dc:title"));
    assert!(!xml.contains("dc:creator"));
}

#[test]
fn a_creator_claims_both_the_dc_and_initial_creator_elements() {
    // A single-author model cannot distinguish "created by" from "last saved
    // by", so it writes both rather than leaving a foreign reader to guess.
    let xml = meta_xml_from(
        &MetaFields {
            creator: Some("Ada"),
            ..MetaFields::default()
        },
        "1.3",
    );
    assert!(xml.contains("<dc:creator>Ada</dc:creator>"));
    assert!(xml.contains("<meta:initial-creator>Ada</meta:initial-creator>"));
}

#[test]
fn text_is_escaped_in_values_and_user_defined_names() {
    let xml = meta_xml_from(
        &MetaFields {
            title: Some("A & B <tag>"),
            user_defined: vec![("dcmi:x\"y".to_string(), "v<z".to_string())],
            ..MetaFields::default()
        },
        "1.3",
    );
    assert!(xml.contains("<dc:title>A &amp; B &lt;tag&gt;</dc:title>"));
    assert!(xml.contains("meta:name=\"dcmi:x&quot;y\""));
    assert!(xml.contains(">v&lt;z</meta:user-defined>"));
    // The raw forms must not survive anywhere in the part.
    assert!(!xml.contains("A & B"));
}

#[test]
fn the_office_version_is_the_one_supplied() {
    // The polarity for the version pass-through: it is not hardcoded to 1.3.
    for v in ["1.1", "1.2", "1.3"] {
        let xml = meta_xml_from(&MetaFields::default(), v);
        assert!(
            xml.contains(&format!("office:version=\"{v}\"")),
            "version {v} must reach the part"
        );
    }
}

#[test]
fn escape_covers_every_xml_significant_character() {
    assert_eq!(escape("&<>\"'"), "&amp;&lt;&gt;&quot;&apos;");
    assert_eq!(escape("plain"), "plain");
}
