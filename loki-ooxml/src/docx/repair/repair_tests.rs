// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the DOCX child-ordering repair pass, exercising the internal
//! DOM + reorder directly on XML fragments (no OPC packaging needed).

use super::*;

/// Parse → walk → serialize a WML fragment; return (output, findings).
fn run(xml: &str, apply: bool) -> (String, Vec<RepairFinding>) {
    let mut nodes = dom::parse(xml.as_bytes()).expect("parse");
    let mut findings = Vec::new();
    reorder_tree(&mut nodes, "test.xml", apply, &mut findings);
    (
        String::from_utf8(dom::serialize(&nodes)).expect("utf8"),
        findings,
    )
}

#[test]
fn ppr_jc_before_spacing_is_reordered() {
    let xml = r#"<w:pPr xmlns:w="w"><w:jc w:val="center"/><w:spacing w:after="0"/></w:pPr>"#;
    let (out, f) = run(xml, true);
    assert_eq!(f.len(), 1, "one violation");
    assert_eq!(f[0].container, "w:pPr");
    let (js, ss) = (out.find("w:jc").unwrap(), out.find("w:spacing").unwrap());
    assert!(ss < js, "spacing must now precede jc: {out}");
}

#[test]
fn rpr_color_must_precede_sz() {
    let xml = r#"<w:rPr xmlns:w="w"><w:i/><w:sz w:val="26"/><w:color w:val="C55A11"/></w:rPr>"#;
    let (out, f) = run(xml, true);
    assert_eq!(f.len(), 1);
    assert!(
        out.find("w:color").unwrap() < out.find("w:sz").unwrap(),
        "{out}"
    );
}

#[test]
fn tcpr_shd_must_precede_valign() {
    let xml = r#"<w:tcPr xmlns:w="w"><w:tcW w:w="1440"/><w:vMerge w:val="restart"/><w:vAlign w:val="center"/><w:shd w:val="clear"/></w:tcPr>"#;
    let (out, f) = run(xml, true);
    assert_eq!(f.len(), 1);
    assert!(
        out.find("w:shd").unwrap() < out.find("w:vAlign").unwrap(),
        "{out}"
    );
    // vMerge stays before shd (both already in order relative to each other).
    assert!(out.find("w:vMerge").unwrap() < out.find("w:shd").unwrap());
}

#[test]
fn already_ordered_is_untouched_and_clean() {
    let xml =
        r#"<w:pPr xmlns:w="w"><w:keepNext/><w:spacing w:after="0"/><w:jc w:val="both"/></w:pPr>"#;
    let (out, f) = run(xml, true);
    assert!(f.is_empty(), "no findings for ordered input");
    assert_eq!(out, xml, "clean input must serialize byte-for-byte");
}

#[test]
fn foreign_child_is_left_alone() {
    // A container holding an element not in the schema table (an extension) is
    // conservatively skipped — we cannot know where to place the unknown child.
    let xml =
        r#"<w:pPr xmlns:w="w" xmlns:x="x"><w:jc w:val="center"/><x:custom/><w:spacing/></w:pPr>"#;
    let (out, f) = run(xml, true);
    assert!(f.is_empty(), "foreign child → no repair");
    assert_eq!(out, xml, "unchanged");
}

#[test]
fn analyze_only_reports_but_does_not_reorder() {
    let xml = r#"<w:pPr xmlns:w="w"><w:jc w:val="center"/><w:spacing/></w:pPr>"#;
    let (out, f) = run(xml, false); // apply = false
    assert_eq!(f.len(), 1, "still reported");
    assert!(
        out.find("w:jc").unwrap() < out.find("w:spacing").unwrap(),
        "order NOT changed"
    );
}

#[test]
fn entities_and_significant_whitespace_are_preserved() {
    // The out-of-order pPr must be fixed WITHOUT disturbing the run text, which
    // carries a preserved space and character/entity references. A naive
    // re-escape would double-encode `&#160;` → `&amp;#160;`.
    let xml = concat!(
        r#"<w:p xmlns:w="w">"#,
        r#"<w:pPr><w:jc w:val="both"/><w:spacing w:after="0"/></w:pPr>"#,
        r#"<w:r><w:t xml:space="preserve">a &amp; b&#160;c </w:t></w:r>"#,
        r#"</w:p>"#,
    );
    let (out, f) = run(xml, true);
    assert_eq!(f.len(), 1);
    assert!(
        out.contains("a &amp; b&#160;c "),
        "text/entities preserved: {out}"
    );
    assert!(out.contains(r#"xml:space="preserve""#), "attr preserved");
    // The pPr got reordered but the run is intact after it.
    assert!(out.find("w:spacing").unwrap() < out.find("w:jc").unwrap());
}

#[test]
fn nested_rpr_in_ppr_is_also_reordered() {
    // Paragraph-mark run props (w:rPr inside w:pPr) are reached by the recursion.
    let xml = concat!(
        r#"<w:pPr xmlns:w="w"><w:spacing/>"#,
        r#"<w:rPr><w:sz w:val="20"/><w:color w:val="FF0000"/></w:rPr>"#,
        r#"</w:pPr>"#,
    );
    let (out, f) = run(xml, true);
    assert_eq!(f.len(), 1, "the inner rPr is out of order");
    assert!(
        out.find("w:color").unwrap() < out.find("w:sz").unwrap(),
        "{out}"
    );
}

#[test]
fn comment_nodes_are_preserved_and_block_reorder() {
    // An XML comment inside a container is significant to a maintainer; we do not
    // reorder around it (conservative) and must not drop it.
    let xml = r#"<w:pPr xmlns:w="w"><w:jc/><!-- note --><w:spacing/></w:pPr>"#;
    let (out, f) = run(xml, true);
    assert!(f.is_empty(), "comment present → skip reorder");
    assert!(out.contains("<!-- note -->"), "comment preserved: {out}");
}
