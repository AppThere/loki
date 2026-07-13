// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `MathML` → OMML for the structured constructs (5.8): fenced expressions
//! (`<mfenced>` → `m:d`), n-ary operators (script elements with an n-ary
//! `<mo>` base → `m:nary`), matrices (`<mtable>` → `m:m`), accents
//! (`<mover accent="true">` → `m:acc`), and limits (`<munder>`/`<mover>` →
//! `m:limLow`/`m:limUpp`). Inverse of `read_structs`.

use std::io::Write;

use quick_xml::Writer;

use super::XmlNode;
use super::write::{empty, end, start, write_arg, write_seq};

/// Whether `node` is a script element whose base is an n-ary operator `<mo>`
/// (`∑`, `∫`, …) — the shape `read_structs::nary` produces. An accented
/// `<mover>` is excluded (that is `m:acc`).
pub(super) fn is_nary_script(node: &XmlNode) -> bool {
    matches!(
        node.tag.as_str(),
        "munderover" | "munder" | "mover" | "msubsup" | "msub" | "msup"
    ) && node.attr("accent") != Some("true")
        && node
            .children
            .first()
            .is_some_and(|c| c.tag == "mo" && is_nary_char(&c.text))
}

/// The big-operator characters that identify an n-ary construct: sums,
/// products, integrals, and the big set/logic operators (ECMA-376 `m:chr`).
fn is_nary_char(s: &str) -> bool {
    let mut chars = s.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return false;
    };
    matches!(c,
        '\u{2211}' | '\u{220F}' | '\u{2210}' // ∑ ∏ ∐
        | '\u{222B}'..='\u{2233}'            // ∫ … ∳ (single/multi/contour)
        | '\u{22C0}'..='\u{22C3}'            // ⋀ ⋁ ⋂ ⋃
        | '\u{2A00}'..='\u{2A06}'            // ⨀ ⨁ ⨂ ⨃ ⨄ ⨅ ⨆
    )
}

/// Emits `m:nary` for a script element with an n-ary `<mo>` base. `operand`
/// is the following sibling (becomes `m:e`); returns whether it was consumed.
pub(super) fn write_nary<W: Write>(
    w: &mut Writer<W>,
    node: &XmlNode,
    operand: Option<&XmlNode>,
) -> bool {
    let (has_lower, has_upper, und_ovr) = match node.tag.as_str() {
        "munderover" => (true, true, true),
        "munder" => (true, false, true),
        "mover" => (false, true, true),
        "msubsup" => (true, true, false),
        "msub" => (true, false, false),
        _ => (false, true, false), // msup
    };
    let chr = node.children.first().map(|c| c.text.as_str()).unwrap_or("");
    start(w, "m:nary", &[]);
    start(w, "m:naryPr", &[]);
    empty(w, "m:chr", &[("m:val", chr)]);
    empty(
        w,
        "m:limLoc",
        &[("m:val", if und_ovr { "undOvr" } else { "subSup" })],
    );
    if !has_lower {
        empty(w, "m:subHide", &[("m:val", "1")]);
    }
    if !has_upper {
        empty(w, "m:supHide", &[("m:val", "1")]);
    }
    end(w, "m:naryPr");
    write_arg(
        w,
        "m:sub",
        has_lower.then(|| node.children.get(1)).flatten(),
    );
    let sup_idx = if has_lower { 2 } else { 1 };
    write_arg(
        w,
        "m:sup",
        has_upper.then(|| node.children.get(sup_idx)).flatten(),
    );
    write_arg(w, "m:e", operand);
    end(w, "m:nary");
    operand.is_some()
}

/// Emits `m:d` for `<mfenced>`: explicit `m:dPr` fence characters (the
/// `MathML` defaults `(`/`)`/`,` when the attributes are absent) and one
/// `m:e` per child.
pub(super) fn write_fenced<W: Write>(w: &mut Writer<W>, node: &XmlNode) {
    start(w, "m:d", &[]);
    start(w, "m:dPr", &[]);
    empty(
        w,
        "m:begChr",
        &[("m:val", node.attr("open").unwrap_or("("))],
    );
    empty(
        w,
        "m:sepChr",
        &[("m:val", node.attr("separators").unwrap_or(","))],
    );
    empty(
        w,
        "m:endChr",
        &[("m:val", node.attr("close").unwrap_or(")"))],
    );
    end(w, "m:dPr");
    for child in &node.children {
        write_arg(w, "m:e", Some(child));
    }
    end(w, "m:d");
}

/// Emits `m:m` for `<mtable>`: `<mtr>` → `m:mr`, `<mtd>` → cell `m:e`.
pub(super) fn write_matrix<W: Write>(w: &mut Writer<W>, node: &XmlNode) {
    start(w, "m:m", &[]);
    for row in node.children.iter().filter(|c| c.tag == "mtr") {
        start(w, "m:mr", &[]);
        for cell in row.children.iter().filter(|c| c.tag == "mtd") {
            start(w, "m:e", &[]);
            write_seq(w, &cell.children);
            end(w, "m:e");
        }
        end(w, "m:mr");
    }
    end(w, "m:m");
}

/// Emits the non-n-ary under/over constructs: an accented `<mover>` becomes
/// `m:acc`; plain `<mover>`/`<munder>` become the `m:limUpp`/`m:limLow`
/// limit elements; a generic `<munderover>` nests `m:limLow` inside
/// `m:limUpp` (best effort — it reads back as `mover(munder(…))`, which is
/// itself round-trip stable).
pub(super) fn write_over_under<W: Write>(w: &mut Writer<W>, node: &XmlNode) {
    match node.tag.as_str() {
        "mover" if node.attr("accent") == Some("true") => {
            let chr = node.children.get(1).map(|c| c.text.as_str()).unwrap_or("");
            start(w, "m:acc", &[]);
            start(w, "m:accPr", &[]);
            empty(w, "m:chr", &[("m:val", chr)]);
            end(w, "m:accPr");
            write_arg(w, "m:e", node.children.first());
            end(w, "m:acc");
        }
        "mover" | "munder" => {
            let elem = if node.tag == "mover" {
                "m:limUpp"
            } else {
                "m:limLow"
            };
            start(w, elem, &[]);
            write_arg(w, "m:e", node.children.first());
            write_arg(w, "m:lim", node.children.get(1));
            end(w, elem);
        }
        _ => {
            // munderover
            start(w, "m:limUpp", &[]);
            start(w, "m:e", &[]);
            start(w, "m:limLow", &[]);
            write_arg(w, "m:e", node.children.first());
            write_arg(w, "m:lim", node.children.get(1));
            end(w, "m:limLow");
            end(w, "m:e");
            write_arg(w, "m:lim", node.children.get(2));
            end(w, "m:limUpp");
        }
    }
}
