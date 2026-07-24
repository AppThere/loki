// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the XML → tree builder: structure, namespaces, attributes, text,
//! entity handling, and totality on malformed input.

use super::parse_dom;
use crate::xml_c14n::C14nNode;

#[test]
fn builds_a_simple_tree_with_attrs_and_text() {
    let root = parse_dom(b"<r a=\"1\"><c>hello</c></r>").unwrap();
    assert_eq!(root.name.local, "r");
    assert_eq!(root.attrs.len(), 1);
    assert_eq!(root.attrs[0].name.local, "a");
    assert_eq!(root.attrs[0].value, "1");
    // One child element <c> with a text node.
    let elems: Vec<_> = root
        .children
        .iter()
        .filter_map(|n| match n {
            C14nNode::Element(e) => Some(e),
            C14nNode::Text(_) => None,
        })
        .collect();
    assert_eq!(elems.len(), 1);
    assert_eq!(elems[0].name.local, "c");
    assert!(matches!(&elems[0].children[0], C14nNode::Text(t) if t == "hello"));
}

#[test]
fn separates_xmlns_declarations_from_attributes() {
    let root = parse_dom(b"<r xmlns=\"urn:d\" xmlns:x=\"urn:x\" id=\"7\"></r>").unwrap();
    assert_eq!(root.attrs.len(), 1, "xmlns:* are not plain attributes");
    assert_eq!(root.attrs[0].name.local, "id");
    assert!(root.ns_decls.contains(&(None, "urn:d".to_owned())));
    assert!(
        root.ns_decls
            .contains(&(Some("x".to_owned()), "urn:x".to_owned()))
    );
}

#[test]
fn keeps_element_prefixes() {
    let root = parse_dom(b"<ds:Signature xmlns:ds=\"urn:d\"></ds:Signature>").unwrap();
    assert_eq!(root.name.prefix.as_deref(), Some("ds"));
    assert_eq!(root.name.local, "Signature");
}

#[test]
fn self_closing_element_has_no_children() {
    let root = parse_dom(b"<r><m Algorithm=\"x\"/></r>").unwrap();
    let C14nNode::Element(m) = root
        .children
        .iter()
        .find(|n| matches!(n, C14nNode::Element(_)))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(m.name.local, "m");
    assert!(m.children.is_empty());
    assert_eq!(m.attrs[0].value, "x");
}

#[test]
fn resolves_predefined_entities_in_text() {
    let root = parse_dom(b"<r>a &amp; b &lt; c</r>").unwrap();
    let text: String = root
        .children
        .iter()
        .filter_map(|n| match n {
            C14nNode::Text(t) => Some(t.clone()),
            C14nNode::Element(_) => None,
        })
        .collect();
    assert_eq!(text, "a & b < c");
}

#[test]
fn normalises_crlf_in_text() {
    let root = parse_dom(b"<r>a\r\nb\rc</r>").unwrap();
    let text: String = root
        .children
        .iter()
        .filter_map(|n| match n {
            C14nNode::Text(t) => Some(t.clone()),
            C14nNode::Element(_) => None,
        })
        .collect();
    assert_eq!(text, "a\nb\nc");
}

#[test]
fn malformed_input_yields_none() {
    assert!(parse_dom(b"").is_none());
    assert!(parse_dom(b"<a><b></a>").is_none());
    assert!(parse_dom(b"not xml").is_none());
    // Two root elements is not a single tree.
    assert!(parse_dom(b"<a></a><b></b>").is_none());
}
