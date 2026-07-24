// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A tiny, total XML → [`C14nElement`] tree builder for the ODF `XMLDSig` parser
//! (8A.4). It reads `macrosignatures.xml` with the workspace's patched
//! `quick-xml` 0.41 and produces the namespace-resolved tree the canonicaliser
//! and the signature navigation walk.
//!
//! It keeps every element's *own* namespace declarations (not the inherited
//! scope); [`crate::odf`] flattens the in-scope set onto the `SignedInfo` apex
//! before canonicalising. Inter-element whitespace is preserved (`trim_text` is
//! off) because it is part of the signed canonical octet stream. Comments, PIs,
//! the XML declaration, and the `DOCTYPE` are dropped (`XMLDSig` canonicalisation
//! omits them). Total: malformed input yields `None`, never a panic (T9).

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::xml_c14n::{C14nAttr, C14nElement, C14nNode, QName};

/// Parses `xml` into a single root [`C14nElement`], or `None` if it is not
/// well-formed enough to yield one root element.
pub(crate) fn parse_dom(xml: &[u8]) -> Option<C14nElement> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut stack: Vec<C14nElement> = Vec::new();
    let mut root: Option<C14nElement> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => stack.push(build_start(e)?),
            Ok(Event::Empty(ref e)) => {
                let el = build_start(e)?;
                attach(el, &mut stack, &mut root)?;
            }
            Ok(Event::End(_)) => {
                let el = stack.pop()?;
                attach(el, &mut stack, &mut root)?;
            }
            Ok(Event::Text(ref t)) => {
                if let Ok(s) = t.decode() {
                    push_text(&normalize_eol(&s), &mut stack);
                }
            }
            Ok(Event::CData(ref c)) => {
                if let Ok(s) = std::str::from_utf8(c) {
                    push_text(&normalize_eol(s), &mut stack);
                }
            }
            Ok(Event::GeneralRef(ref r)) => {
                if let Some(s) = resolve_ref(r) {
                    push_text(&s, &mut stack);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {} // Decl / Comment / PI / DocType — omitted by c14n.
            Err(_) => return None,
        }
        buf.clear();
    }
    // A well-formed document leaves nothing open.
    if stack.is_empty() { root } else { None }
}

/// Builds a childless element from a start (or empty) tag: its qualified name,
/// its own `xmlns`/`xmlns:*` declarations, and its non-namespace attributes.
fn build_start(e: &BytesStart<'_>) -> Option<C14nElement> {
    let name = qname_of(e.name().as_ref())?;
    let mut ns_decls = Vec::new();
    let mut attrs = Vec::new();
    for attr in e.attributes() {
        let Ok(attr) = attr else { return None };
        let key = attr.key.as_ref().to_vec();
        let Ok(value) = attr.normalized_value(quick_xml::XmlVersion::Implicit1_0) else {
            return None;
        };
        let value = value.into_owned();
        if key == b"xmlns" {
            ns_decls.push((None, value));
        } else if let Some(prefix) = key.strip_prefix(b"xmlns:") {
            ns_decls.push((Some(bytes_to_string(prefix)?), value));
        } else {
            attrs.push(C14nAttr {
                name: qname_of(&key)?,
                value,
            });
        }
    }
    Some(C14nElement {
        name,
        ns_decls,
        attrs,
        children: Vec::new(),
    })
}

/// Splits a raw `prefix:local` (or bare `local`) name into a [`QName`].
fn qname_of(raw: &[u8]) -> Option<QName> {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => Some(QName {
            prefix: Some(bytes_to_string(&raw[..i])?),
            local: bytes_to_string(&raw[i + 1..])?,
        }),
        None => Some(QName {
            prefix: None,
            local: bytes_to_string(raw)?,
        }),
    }
}

fn bytes_to_string(b: &[u8]) -> Option<String> {
    std::str::from_utf8(b).ok().map(str::to_owned)
}

/// Attaches a finished element as a child of the open element, or as the root.
/// Returns `None` if a second root would be produced (not well-formed).
fn attach(
    el: C14nElement,
    stack: &mut [C14nElement],
    root: &mut Option<C14nElement>,
) -> Option<()> {
    if let Some(top) = stack.last_mut() {
        top.children.push(C14nNode::Element(el));
        Some(())
    } else if root.is_none() {
        *root = Some(el);
        Some(())
    } else {
        None
    }
}

/// Appends character data to the currently-open element (text outside any
/// element — e.g. leading whitespace — is discarded).
fn push_text(text: &str, stack: &mut [C14nElement]) {
    if let Some(top) = stack.last_mut() {
        top.children.push(C14nNode::Text(text.to_owned()));
    }
}

/// Normalises XML line endings (`\r\n` and lone `\r` → `\n`) as the XML processor
/// would before canonicalisation.
fn normalize_eol(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Resolves an entity/character reference to its text: the five predefined XML
/// entities and numeric refs; an unknown named entity degrades to its literal
/// `&name;` so nothing is silently dropped.
fn resolve_ref(r: &quick_xml::events::BytesRef<'_>) -> Option<String> {
    if let Ok(Some(ch)) = r.resolve_char_ref() {
        return Some(ch.to_string());
    }
    let name = r.decode().ok()?;
    Some(
        quick_xml::escape::resolve_predefined_entity(&name)
            .map_or_else(|| format!("&{name};"), ToString::to_string),
    )
}

#[cfg(test)]
#[path = "odf_dom_tests.rs"]
mod tests;
