// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Embedded math (formula) objects for ODT.
//!
//! ODF stores mathematical content as an embedded formula sub-document: a
//! `draw:frame`/`draw:object` in `content.xml` references an `Object N/`
//! directory whose `content.xml` holds a `MathML` `<math>` document (ODF 1.3
//! §3.16, §12). The format-neutral model stores that same `MathML` string in
//! [`loki_doc_model::content::inline::Inline::Math`].
//!
//! This module renders the object's `content.xml` on export and extracts +
//! canonicalises the `MathML` on import, so a formula round-trips byte-for-byte
//! through the model.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::xml_util::{resolve_general_ref, unescape_text};

/// The `MathML` namespace URI placed on the root `<math>` element.
const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";

/// Builds the `content.xml` body of a formula object from a `MathML` string.
#[must_use]
pub(crate) fn object_content_xml(mathml: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{mathml}")
}

/// Extracts the `MathML` `<math>` document from a formula object's `content.xml`
/// bytes, re-serialising it into the model's canonical form (unprefixed tags,
/// the `MathML` namespace on the root, no insignificant whitespace).
///
/// Returns `None` if no `math` root element is found.
#[must_use]
pub(crate) fn extract_mathml(object_xml: &[u8]) -> Option<String> {
    let root = read_math_root(object_xml)?;
    Some(serialize(&root, true))
}

/// A minimal XML element tree used to canonicalise the parsed `MathML`.
#[derive(Default)]
struct Node {
    tag: String,
    text: String,
    children: Vec<Node>,
}

/// Parses `bytes`, returning the first `math` element as a tree.
fn read_math_root(bytes: &[u8]) -> Option<Node> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = local_tag(e.name().as_ref());
                if tag == "math" {
                    return Some(read_node(&mut reader, &tag));
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

/// Reads the children of an element named `tag` (its `Start` already consumed)
/// until the matching `End`.
fn read_node(reader: &mut Reader<&[u8]>, tag: &str) -> Node {
    let mut node = Node {
        tag: tag.to_string(),
        ..Default::default()
    };
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let ctag = local_tag(e.name().as_ref());
                node.children.push(read_node(reader, &ctag));
            }
            Ok(Event::Empty(ref e)) => node.children.push(Node {
                tag: local_tag(e.name().as_ref()),
                ..Default::default()
            }),
            Ok(Event::Text(ref t)) => {
                if let Ok(s) = unescape_text(t) {
                    node.text.push_str(&s);
                }
            }
            Ok(Event::GeneralRef(ref r)) => {
                if let Ok(s) = resolve_general_ref(r) {
                    node.text.push_str(&s);
                }
            }
            Ok(Event::End(ref e)) => {
                if local_tag(e.name().as_ref()) == tag {
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    node
}

/// Canonically serialises a node; the root carries the `MathML` namespace.
fn serialize(node: &Node, is_root: bool) -> String {
    let attrs = if is_root {
        format!(" xmlns=\"{MATHML_NS}\"")
    } else {
        String::new()
    };
    if node.children.is_empty() {
        if node.text.is_empty() {
            format!("<{0}{1}/>", node.tag, attrs)
        } else {
            format!("<{0}{1}>{2}</{0}>", node.tag, attrs, escape(&node.text))
        }
    } else {
        let inner: String = node.children.iter().map(|c| serialize(c, false)).collect();
        format!("<{0}{1}>{2}</{0}>", node.tag, attrs, inner)
    }
}

/// Local name (prefix stripped) of an element name as an owned string.
fn local_tag(name: &[u8]) -> String {
    let local = match name.iter().rposition(|&b| b == b':') {
        Some(pos) => &name[pos + 1..],
        None => name,
    };
    String::from_utf8_lossy(local).into_owned()
}

/// Escapes the five XML predefined entities, matching the OOXML converter so
/// `MathML` produced by either format normalises identically.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
#[path = "math_tests.rs"]
mod tests;
