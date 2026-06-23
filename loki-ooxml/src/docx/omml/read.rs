// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OMML → `MathML`: reads an `m:oMath` / `m:oMathPara` subtree from the live
//! document reader and converts it to a `MathML` string. ECMA-376 §22.1.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::{MATHML_NS, XmlNode, escape_xml};
use crate::docx::reader::util::local_name;
use crate::error::{OoxmlError, OoxmlResult};

/// Reads the math element whose `Start` event (`start`) has just been consumed
/// by the paragraph reader, returning `(mathml, is_display)`.
///
/// `m:oMathPara` denotes display math (its child `m:oMath` carries the body);
/// a bare `m:oMath` is inline math.
pub(crate) fn read_math(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart,
) -> OoxmlResult<(String, bool)> {
    let root_tag = local_tag(start.name().as_ref());
    let node = read_node(reader, &root_tag)?;
    let display = root_tag == "oMathPara";
    let omath = if display {
        node.child("oMath").cloned().unwrap_or(node)
    } else {
        node
    };
    let body: String = omath.children.iter().filter_map(convert).collect();
    Ok((
        format!("<math xmlns=\"{MATHML_NS}\">{body}</math>"),
        display,
    ))
}

/// Returns the prefix-stripped local name of an element name as an owned string.
fn local_tag(name: &[u8]) -> String {
    String::from_utf8_lossy(local_name(name)).into_owned()
}

/// Reads the children of an element named `tag` (whose `Start` was already
/// consumed) until the matching `End`, returning the assembled node.
///
/// Shared by the OMML reader and the MathML-string parser used on export.
pub(super) fn read_node(reader: &mut Reader<&[u8]>, tag: &str) -> OoxmlResult<XmlNode> {
    let mut node = XmlNode {
        tag: tag.to_string(),
        ..Default::default()
    };
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let ctag = local_tag(e.name().as_ref());
                node.children.push(read_node(reader, &ctag)?);
            }
            Ok(Event::Empty(ref e)) => {
                node.children.push(XmlNode {
                    tag: local_tag(e.name().as_ref()),
                    ..Default::default()
                });
            }
            Ok(Event::Text(ref t)) => {
                if let Ok(s) = t.unescape() {
                    node.text.push_str(&s);
                }
            }
            Ok(Event::End(ref e)) => {
                if local_tag(e.name().as_ref()) == tag {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(node)
}

// ── OMML node → MathML fragment ─────────────────────────────────────────────

/// Converts one OMML content node to a `MathML` fragment, or `None` for property
/// elements and empty content.
fn convert(node: &XmlNode) -> Option<String> {
    match node.tag.as_str() {
        "r" => {
            let text: String = node
                .children
                .iter()
                .filter(|c| c.tag == "t")
                .map(|c| c.text.as_str())
                .collect();
            token(&text)
        }
        "f" => Some(format!(
            "<mfrac>{}{}</mfrac>",
            wrapper(node, "num"),
            wrapper(node, "den")
        )),
        "sSup" => Some(format!(
            "<msup>{}{}</msup>",
            wrapper(node, "e"),
            wrapper(node, "sup")
        )),
        "sSub" => Some(format!(
            "<msub>{}{}</msub>",
            wrapper(node, "e"),
            wrapper(node, "sub")
        )),
        "sSubSup" => Some(format!(
            "<msubsup>{}{}{}</msubsup>",
            wrapper(node, "e"),
            wrapper(node, "sub"),
            wrapper(node, "sup")
        )),
        "rad" => {
            let base = wrapper(node, "e");
            let deg_empty = node
                .child("deg")
                .is_none_or(|d| seq(&d.children).is_empty());
            if deg_empty {
                Some(format!("<msqrt>{base}</msqrt>"))
            } else {
                Some(format!("<mroot>{base}{}</mroot>", wrapper(node, "deg")))
            }
        }
        // Property and text-leaf elements carry no standalone math content.
        "rPr" | "fPr" | "sSupPr" | "sSubPr" | "sSubSupPr" | "radPr" | "ctrlPr" | "t" => None,
        // Unknown container: emit its content (best effort).
        _ => {
            let inner = seq(&node.children).concat();
            (!inner.is_empty()).then_some(inner)
        }
    }
}

/// Converts a sequence of nodes, dropping the ones with no math content.
fn seq(nodes: &[XmlNode]) -> Vec<String> {
    nodes.iter().filter_map(convert).collect()
}

/// Converts the content of `parent`'s child named `tag` into a single `MathML`
/// argument, wrapping multiple elements in an `<mrow>`.
fn wrapper(parent: &XmlNode, tag: &str) -> String {
    let parts = parent
        .child(tag)
        .map(|c| seq(&c.children))
        .unwrap_or_default();
    mrow(parts)
}

/// Groups `parts` into a single `MathML` element: a lone element is returned as
/// is; zero or several are wrapped in `<mrow>`.
fn mrow(parts: Vec<String>) -> String {
    match parts.len() {
        0 => "<mrow/>".to_string(),
        1 => parts.into_iter().next().unwrap_or_default(),
        _ => format!("<mrow>{}</mrow>", parts.concat()),
    }
}

/// Classifies a math run's text into the appropriate `MathML` token element:
/// `<mn>` for numerals, `<mi>` for identifiers, `<mo>` otherwise.
fn token(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    let esc = escape_xml(s);
    if s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        Some(format!("<mn>{esc}</mn>"))
    } else if s.chars().all(char::is_alphabetic) {
        Some(format!("<mi>{esc}</mi>"))
    } else {
        Some(format!("<mo>{esc}</mo>"))
    }
}
