// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OMML → `MathML` for the structured constructs (5.8): delimiters (`m:d`),
//! n-ary operators (`m:nary`), matrices (`m:m`), and accents (`m:acc`).
//! Each mapping is the exact inverse of the corresponding `write_structs`
//! emitter, so the round-trip is stable (`assert_stable` in `omml_tests`).

use super::read::{mrow, seq, wrapper};
use super::{XmlNode, escape_xml};

/// Looks up `<{pr}><{child} m:val="…"/></{pr}>` on `node`.
fn pr_val<'n>(node: &'n XmlNode, pr: &str, child: &str) -> Option<&'n str> {
    node.child(pr)?.child(child)?.attr("val")
}

/// Whether the OOXML on/off flag `<{pr}><{child}/></{pr}>` is set: present
/// with no `m:val`, or a value other than `0`/`false`/`off`.
fn pr_flag(node: &XmlNode, pr: &str, child: &str) -> bool {
    match node.child(pr).and_then(|p| p.child(child)) {
        None => false,
        Some(c) => !matches!(c.attr("val"), Some("0" | "false" | "off")),
    }
}

/// `m:d` → `<mfenced open close separators>` with one child per `m:e`.
/// The fence characters are resolved (OMML defaults `(`, `)`, `|`) and always
/// written explicitly, so the writer's inverse needs no default juggling.
pub(super) fn delim(node: &XmlNode) -> String {
    let open = pr_val(node, "dPr", "begChr").unwrap_or("(");
    let close = pr_val(node, "dPr", "endChr").unwrap_or(")");
    let sep = pr_val(node, "dPr", "sepChr").unwrap_or("|");
    let args: String = node
        .children
        .iter()
        .filter(|c| c.tag == "e")
        .map(|e| mrow(seq(&e.children)))
        .collect();
    format!(
        "<mfenced open=\"{}\" close=\"{}\" separators=\"{}\">{args}</mfenced>",
        escape_xml(open),
        escape_xml(close),
        escape_xml(sep)
    )
}

/// `m:nary` → an operator script element (`<munderover>`/`<msubsup>` family,
/// per `m:limLoc` and the hidden flags) whose base is the n-ary `<mo>`,
/// followed by the operand (`m:e`) as a sibling. Default operator: `∫`.
pub(super) fn nary(node: &XmlNode) -> String {
    let chr = pr_val(node, "naryPr", "chr").unwrap_or("\u{222B}");
    let und_ovr = pr_val(node, "naryPr", "limLoc").unwrap_or("undOvr") != "subSup";
    let lower = limit_arg(node, "sub", pr_flag(node, "naryPr", "subHide"));
    let upper = limit_arg(node, "sup", pr_flag(node, "naryPr", "supHide"));
    let op = format!("<mo>{}</mo>", escape_xml(chr));
    let script = match (lower, upper) {
        (Some(s), Some(p)) if und_ovr => format!("<munderover>{op}{s}{p}</munderover>"),
        (Some(s), Some(p)) => format!("<msubsup>{op}{s}{p}</msubsup>"),
        (Some(s), None) if und_ovr => format!("<munder>{op}{s}</munder>"),
        (Some(s), None) => format!("<msub>{op}{s}</msub>"),
        (None, Some(p)) if und_ovr => format!("<mover>{op}{p}</mover>"),
        (None, Some(p)) => format!("<msup>{op}{p}</msup>"),
        (None, None) => op,
    };
    let operand = node
        .child("e")
        .map(|e| seq(&e.children))
        .unwrap_or_default();
    if operand.is_empty() {
        script
    } else {
        format!("{script}{}", mrow(operand))
    }
}

/// A hidden or contentless n-ary limit converts to `None` (the script element
/// degrades to the fewer-argument form); otherwise one grouped element.
fn limit_arg(node: &XmlNode, tag: &str, hidden: bool) -> Option<String> {
    if hidden {
        return None;
    }
    let parts = node
        .child(tag)
        .map(|c| seq(&c.children))
        .unwrap_or_default();
    (!parts.is_empty()).then(|| mrow(parts))
}

/// `m:m` → `<mtable>`: each `m:mr` becomes `<mtr>`, each cell `m:e` `<mtd>`.
pub(super) fn matrix(node: &XmlNode) -> String {
    let mut out = String::from("<mtable>");
    for r in node.children.iter().filter(|c| c.tag == "mr") {
        out.push_str("<mtr>");
        for e in r.children.iter().filter(|c| c.tag == "e") {
            out.push_str("<mtd>");
            out.push_str(&seq(&e.children).concat());
            out.push_str("</mtd>");
        }
        out.push_str("</mtr>");
    }
    out.push_str("</mtable>");
    out
}

/// `m:acc` → `<mover accent="true">base<mo>chr</mo></mover>`. Default accent
/// character: U+0302 COMBINING CIRCUMFLEX ACCENT (the OMML default).
pub(super) fn accent(node: &XmlNode) -> String {
    let chr = pr_val(node, "accPr", "chr").unwrap_or("\u{0302}");
    format!(
        "<mover accent=\"true\">{}<mo>{}</mo></mover>",
        wrapper(node, "e"),
        escape_xml(chr)
    )
}
