// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Canonical XML 1.0 tests. Expected outputs are hand-derived from the W3C
//! `REC-xml-c14n-20010315` rules for the element/attribute/namespace/text subset
//! that appears inside an `XMLDSig` `SignedInfo`. Real-signer interop
//! (`LibreOffice` / Word) is the `TODO(8A.4-corpus)` gate.

use super::{C14nAttr, C14nElement, C14nNode, QName, canonicalize};

const DSIG: &str = "http://www.w3.org/2000/09/xmldsig#";

fn qn(local: &str) -> QName {
    QName {
        prefix: None,
        local: local.to_owned(),
    }
}

fn attr(local: &str, value: &str) -> C14nAttr {
    C14nAttr {
        name: qn(local),
        value: value.to_owned(),
    }
}

fn elem(local: &str) -> C14nElement {
    C14nElement {
        name: qn(local),
        ns_decls: Vec::new(),
        attrs: Vec::new(),
        children: Vec::new(),
    }
}

fn c14n(el: &C14nElement) -> String {
    String::from_utf8(canonicalize(el)).unwrap()
}

#[test]
fn default_namespace_is_hoisted_onto_the_apex_only() {
    let mut method = elem("CanonicalizationMethod");
    method.attrs.push(attr(
        "Algorithm",
        "http://www.w3.org/TR/2001/REC-xml-c14n-20010315",
    ));
    let mut signed_info = elem("SignedInfo");
    signed_info.ns_decls.push((None, DSIG.to_owned()));
    signed_info.children.push(C14nNode::Element(method));

    // xmlns is rendered on SignedInfo (in scope, no output ancestor rendered it)
    // and NOT repeated on the child; the empty child becomes a start+end pair.
    assert_eq!(
        c14n(&signed_info),
        "<SignedInfo xmlns=\"http://www.w3.org/2000/09/xmldsig#\">\
<CanonicalizationMethod Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\">\
</CanonicalizationMethod></SignedInfo>"
    );
}

#[test]
fn attributes_sort_by_local_name() {
    let mut reference = elem("Reference");
    reference
        .attrs
        .push(attr("URI", "Basic/Standard/Module1.xml"));
    reference.attrs.push(attr("Id", "r1"));
    // Id sorts before URI regardless of source order.
    assert_eq!(
        c14n(&reference),
        "<Reference Id=\"r1\" URI=\"Basic/Standard/Module1.xml\"></Reference>"
    );
}

#[test]
fn unprefixed_attributes_precede_prefixed_ones() {
    let mut el = elem("E");
    el.ns_decls.push((Some("x".to_owned()), "urn:x".to_owned()));
    el.attrs.push(C14nAttr {
        name: QName {
            prefix: Some("x".to_owned()),
            local: "a".to_owned(),
        },
        value: "1".to_owned(),
    });
    el.attrs.push(attr("b", "2"));
    // No-namespace `b` (empty URI) sorts before `x:a` (urn:x).
    assert_eq!(c14n(&el), "<E xmlns:x=\"urn:x\" b=\"2\" x:a=\"1\"></E>");
}

#[test]
fn text_escapes_amp_lt_gt_but_not_quote() {
    let mut el = elem("DigestValue");
    el.children.push(C14nNode::Text("a<b>&c\"d".to_owned()));
    assert_eq!(c14n(&el), "<DigestValue>a&lt;b&gt;&amp;c\"d</DigestValue>");
}

#[test]
fn attr_value_escapes_quote_amp_and_whitespace_but_not_gt() {
    let mut el = elem("E");
    el.attrs.push(attr("v", "a&b\"c>d\te\nf"));
    assert_eq!(c14n(&el), "<E v=\"a&amp;b&quot;c>d&#x9;e&#xA;f\"></E>");
}

#[test]
fn child_redeclaring_the_same_default_namespace_is_dropped() {
    let mut child = elem("Reference");
    child.ns_decls.push((None, DSIG.to_owned())); // redundant
    let mut parent = elem("SignedInfo");
    parent.ns_decls.push((None, DSIG.to_owned()));
    parent.children.push(C14nNode::Element(child));
    assert_eq!(
        c14n(&parent),
        "<SignedInfo xmlns=\"http://www.w3.org/2000/09/xmldsig#\">\
<Reference></Reference></SignedInfo>"
    );
}

#[test]
fn namespace_nodes_render_default_first_then_by_prefix() {
    let mut el = elem("SignedInfo");
    el.ns_decls.push((Some("ds".to_owned()), DSIG.to_owned()));
    el.ns_decls.push((None, "urn:oasis:sig".to_owned()));
    el.ns_decls.push((Some("a".to_owned()), "urn:a".to_owned()));
    // Default (xmlns) first, then prefixes a, ds lexicographically.
    assert_eq!(
        c14n(&el),
        "<SignedInfo xmlns=\"urn:oasis:sig\" xmlns:a=\"urn:a\" xmlns:ds=\"http://www.w3.org/2000/09/xmldsig#\"></SignedInfo>"
    );
}

#[test]
fn prefixed_element_keeps_its_qualified_name() {
    let mut el = C14nElement {
        name: QName {
            prefix: Some("ds".to_owned()),
            local: "SignedInfo".to_owned(),
        },
        ns_decls: vec![(Some("ds".to_owned()), DSIG.to_owned())],
        attrs: Vec::new(),
        children: Vec::new(),
    };
    el.children.push(C14nNode::Text(String::new()));
    assert_eq!(
        c14n(&el),
        "<ds:SignedInfo xmlns:ds=\"http://www.w3.org/2000/09/xmldsig#\"></ds:SignedInfo>"
    );
}

#[test]
fn carriage_return_in_text_is_escaped() {
    let mut el = elem("T");
    el.children.push(C14nNode::Text("a\r\nb".to_owned()));
    // #xD escaped, #xA kept literally.
    assert_eq!(c14n(&el), "<T>a&#xD;\nb</T>");
}
