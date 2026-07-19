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

// ── mc:Ignorable (undeclared-prefix) repair ──────────────────────────────────

/// Parse → mc:Ignorable fix → serialize; return (output, findings).
fn run_mce(xml: &str, apply: bool) -> (String, Vec<RepairFinding>) {
    let mut nodes = dom::parse(xml.as_bytes()).expect("parse");
    let mut findings = Vec::new();
    mce::fix_ignorable_tree(&mut nodes, "test.xml", apply, &mut findings);
    (
        String::from_utf8(dom::serialize(&nodes)).expect("utf8"),
        findings,
    )
}

#[test]
fn undeclared_ignorable_prefix_is_stripped_declared_one_kept() {
    // The real ACID2 bug: `mc:Ignorable="w14"` with no `xmlns:w14`. Here w15 is
    // declared and must survive; w14 is not and must be removed.
    let xml = concat!(
        r#"<w:styles xmlns:w="w" xmlns:mc="mc" "#,
        r#"xmlns:w15="w15" mc:Ignorable="w14 w15"/>"#,
    );
    let (out, f) = run_mce(xml, true);
    assert_eq!(f.len(), 1, "one violation");
    assert_eq!(f[0].container, "mc:Ignorable");
    assert!(
        f[0].detail.contains("w14"),
        "names the dropped prefix: {out}"
    );
    assert!(out.contains(r#"mc:Ignorable="w15""#), "w14 stripped: {out}");
    assert!(!out.contains("w14"), "no trace of w14 remains: {out}");
}

#[test]
fn fully_declared_ignorable_is_untouched_and_clean() {
    // Matches Word's own output (and the fixed ACID2 styles.xml): the prefix is
    // declared, so there is nothing to fix and the bytes are preserved exactly.
    let xml = r#"<w:styles xmlns:w="w" xmlns:mc="mc" xmlns:w14="w14" mc:Ignorable="w14"/>"#;
    let (out, f) = run_mce(xml, true);
    assert!(f.is_empty(), "declared prefix → no finding");
    assert_eq!(out, xml, "clean input serializes byte-for-byte");
}

#[test]
fn stripping_the_only_prefix_removes_the_whole_attribute() {
    // Nothing left to ignore → drop `mc:Ignorable` entirely, without leaving a
    // doubled space, and preserve the surrounding attributes byte-for-byte.
    let xml = r#"<w:styles xmlns:w="w" xmlns:mc="mc" mc:Ignorable="w14"/>"#;
    let (out, f) = run_mce(xml, true);
    assert_eq!(f.len(), 1);
    assert!(!out.contains("mc:Ignorable"), "attribute dropped: {out}");
    assert!(out.contains(r#"xmlns:mc="mc""#), "other attrs kept: {out}");
    assert!(!out.contains("  "), "no doubled whitespace: {out}");
}

#[test]
fn ignorable_analyze_only_reports_but_does_not_edit() {
    let xml = r#"<w:styles xmlns:w="w" xmlns:mc="mc" mc:Ignorable="w14"/>"#;
    let (out, f) = run_mce(xml, false); // apply = false
    assert_eq!(f.len(), 1, "still reported");
    assert!(
        out.contains(r#"mc:Ignorable="w14""#),
        "value NOT changed: {out}"
    );
}

#[test]
fn ignorable_prefix_declared_on_ancestor_resolves() {
    // Scope threading: a prefix declared on the root covers `mc:Ignorable` on a
    // descendant, so it must NOT be flagged.
    let xml = concat!(
        r#"<w:document xmlns:w="w" xmlns:mc="mc" xmlns:w14="w14">"#,
        r#"<w:body mc:Ignorable="w14"/>"#,
        r#"</w:document>"#,
    );
    let (_out, f) = run_mce(xml, true);
    assert!(f.is_empty(), "ancestor-declared prefix resolves");
}
