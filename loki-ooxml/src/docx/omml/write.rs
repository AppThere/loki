// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `MathML` → OMML: parses the model's `MathML` string and emits the corresponding
//! `m:oMath` / `m:oMathPara` OOXML. ECMA-376 §22.1.

use std::io::Write;

use quick_xml::Reader;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Writer, name::QName};

use super::XmlNode;
use super::read::read_node;

/// Writes `mathml` as OMML into the document writer. `display` selects the
/// `m:oMathPara` (block) wrapper; otherwise a bare inline `m:oMath` is emitted.
pub(crate) fn write_omath<W: Write>(w: &mut Writer<W>, mathml: &str, display: bool) {
    let Some(root) = parse_mathml(mathml) else {
        return;
    };
    if display {
        start(w, "m:oMathPara", &[]);
        start(w, "m:oMath", &[]);
        write_seq(w, &root.children);
        end(w, "m:oMath");
        end(w, "m:oMathPara");
    } else {
        start(w, "m:oMath", &[]);
        write_seq(w, &root.children);
        end(w, "m:oMath");
    }
}

/// Parses a `MathML` string into the generic node tree, returning the root
/// `<math>` element.
fn parse_mathml(s: &str) -> Option<XmlNode> {
    let mut reader = Reader::from_reader(s.as_bytes());
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = local_tag(e.name());
                let attrs = super::read::attrs_of(e);
                return read_node(&mut reader, &tag, attrs).ok();
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => buf.clear(),
        }
    }
}

/// Local name of a qualified name as an owned string.
fn local_tag(name: QName) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).into_owned()
}

// ── MathML node → OMML ──────────────────────────────────────────────────────

/// Writes a node sequence. An n-ary script element consumes the following
/// sibling as its `m:e` operand (the inverse of `read_structs::nary`, which
/// emits the operand as a sibling of the operator script).
pub(super) fn write_seq<W: Write>(w: &mut Writer<W>, nodes: &[XmlNode]) {
    let mut i = 0;
    while i < nodes.len() {
        let node = &nodes[i];
        if super::write_structs::is_nary_script(node) {
            let consumed = super::write_structs::write_nary(w, node, nodes.get(i + 1));
            i += 1 + usize::from(consumed);
        } else {
            write_node(w, node);
            i += 1;
        }
    }
}

pub(super) fn write_node<W: Write>(w: &mut Writer<W>, node: &XmlNode) {
    // In argument position there is no sibling operand to consume.
    if super::write_structs::is_nary_script(node) {
        super::write_structs::write_nary(w, node, None);
        return;
    }
    match node.tag.as_str() {
        "mi" | "mn" | "mo" | "mtext" => write_run(w, &node.text),
        "mfrac" => {
            start(w, "m:f", &[]);
            write_arg(w, "m:num", node.children.first());
            write_arg(w, "m:den", node.children.get(1));
            end(w, "m:f");
        }
        "msup" => write_scripts(w, "m:sSup", &[("m:e", 0), ("m:sup", 1)], node),
        "msub" => write_scripts(w, "m:sSub", &[("m:e", 0), ("m:sub", 1)], node),
        "msubsup" => {
            write_scripts(
                w,
                "m:sSubSup",
                &[("m:e", 0), ("m:sub", 1), ("m:sup", 2)],
                node,
            );
        }
        "msqrt" => {
            start(w, "m:rad", &[]);
            start(w, "m:radPr", &[]);
            empty(w, "m:degHide", &[("m:val", "1")]);
            end(w, "m:radPr");
            empty(w, "m:deg", &[]);
            start(w, "m:e", &[]);
            write_seq(w, &node.children);
            end(w, "m:e");
            end(w, "m:rad");
        }
        "mroot" => {
            start(w, "m:rad", &[]);
            write_arg(w, "m:deg", node.children.get(1));
            write_arg(w, "m:e", node.children.first());
            end(w, "m:rad");
        }
        // Structured constructs (5.8) live in `write_structs`.
        "mfenced" => super::write_structs::write_fenced(w, node),
        "mtable" => super::write_structs::write_matrix(w, node),
        "mover" | "munder" | "munderover" => super::write_structs::write_over_under(w, node),
        // `mrow` and unknown wrappers (math, mstyle, semantics, …): OMML has no
        // explicit grouping element, so pass the content through directly.
        _ => write_seq(w, &node.children),
    }
}

/// Emits an OMML script element with each `(omml_tag, child_index)` argument.
fn write_scripts<W: Write>(w: &mut Writer<W>, elem: &str, args: &[(&str, usize)], node: &XmlNode) {
    start(w, elem, &[]);
    for (tag, idx) in args {
        write_arg(w, tag, node.children.get(*idx));
    }
    end(w, elem);
}

/// Writes `<omml_tag>…</omml_tag>` around the OMML for `child` (flattening an
/// `<mrow>`).
pub(super) fn write_arg<W: Write>(w: &mut Writer<W>, tag: &str, child: Option<&XmlNode>) {
    start(w, tag, &[]);
    if let Some(c) = child {
        if c.tag == "mrow" {
            write_seq(w, &c.children);
        } else {
            write_node(w, c);
        }
    }
    end(w, tag);
}

/// Writes a math run `<m:r><m:t xml:space="preserve">text</m:t></m:r>`.
fn write_run<W: Write>(w: &mut Writer<W>, text: &str) {
    start(w, "m:r", &[]);
    start(w, "m:t", &[("xml:space", "preserve")]);
    let _ = w.write_event(Event::Text(BytesText::new(text)));
    end(w, "m:t");
    end(w, "m:r");
}

// ── quick-xml convenience wrappers ──────────────────────────────────────────

pub(super) fn start<W: Write>(w: &mut Writer<W>, tag: &str, attrs: &[(&str, &str)]) {
    let mut e = BytesStart::new(tag);
    for (k, v) in attrs {
        e.push_attribute((*k, *v));
    }
    let _ = w.write_event(Event::Start(e));
}

pub(super) fn end<W: Write>(w: &mut Writer<W>, tag: &str) {
    let _ = w.write_event(Event::End(BytesEnd::new(tag)));
}

pub(super) fn empty<W: Write>(w: &mut Writer<W>, tag: &str, attrs: &[(&str, &str)]) {
    let mut e = BytesStart::new(tag);
    for (k, v) in attrs {
        e.push_attribute((*k, *v));
    }
    let _ = w.write_event(Event::Empty(e));
}
