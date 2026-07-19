// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A tiny, lossless XML tree used only by the repair pass.
//!
//! `quick-xml` is an event streamer with no DOM, but reordering an element's
//! children needs random access to a subtree. This module parses a part into an
//! owned [`Node`] tree and serialises it back. It preserves declarations,
//! comments, processing instructions, CDATA, entity references, attributes, and
//! significant text verbatim — the repair pass only reorders element children,
//! never rewrites content.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};

use crate::error::OoxmlError;

/// One node in the tree.
pub(super) enum Node {
    /// An element: its start tag (name + attributes), children, and whether it
    /// was written self-closing (`<x/>`).
    Elem(Elem),
    /// Any non-element event — text, comment, CDATA, PI, XML declaration,
    /// doctype, or entity reference — carried verbatim.
    Leaf(Event<'static>),
}

/// An element node.
pub(super) struct Elem {
    pub start: BytesStart<'static>,
    pub children: Vec<Node>,
    pub self_closing: bool,
}

impl Elem {
    /// The element's local name (namespace prefix stripped) as a string.
    pub fn local(&self) -> String {
        String::from_utf8_lossy(self.start.name().local_name().as_ref()).into_owned()
    }
}

/// Parses `xml` into a flat list of top-level nodes (declaration + root).
pub(super) fn parse(xml: &[u8]) -> Result<Vec<Node>, OoxmlError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    reader.config_mut().expand_empty_elements = false;
    reader.config_mut().check_end_names = false;

    let mut stack: Vec<(BytesStart<'static>, Vec<Node>)> = Vec::new();
    let mut roots: Vec<Node> = Vec::new();
    let mut buf = Vec::new();
    loop {
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|source| OoxmlError::Xml {
                part: "repair".into(),
                source,
            })?;
        match ev {
            Event::Start(e) => stack.push((e.into_owned(), Vec::new())),
            Event::End(_) => {
                let (start, children) =
                    stack.pop().ok_or_else(|| OoxmlError::MalformedElement {
                        element: String::new(),
                        part: "repair".into(),
                        reason: "unbalanced end tag".into(),
                    })?;
                push(
                    &mut stack,
                    &mut roots,
                    Node::Elem(Elem {
                        start,
                        children,
                        self_closing: false,
                    }),
                );
            }
            Event::Empty(e) => push(
                &mut stack,
                &mut roots,
                Node::Elem(Elem {
                    start: e.into_owned(),
                    children: Vec::new(),
                    self_closing: true,
                }),
            ),
            Event::Eof => break,
            other => push(&mut stack, &mut roots, Node::Leaf(other.into_owned())),
        }
        buf.clear();
    }
    if !stack.is_empty() {
        return Err(OoxmlError::MalformedElement {
            element: String::new(),
            part: "repair".into(),
            reason: "unclosed element".into(),
        });
    }
    Ok(roots)
}

fn push(stack: &mut [(BytesStart<'static>, Vec<Node>)], roots: &mut Vec<Node>, node: Node) {
    if let Some((_, children)) = stack.last_mut() {
        children.push(node);
    } else {
        roots.push(node);
    }
}

/// Serialises a node list back to XML bytes.
pub(super) fn serialize(nodes: &[Node]) -> Vec<u8> {
    let mut w = Writer::new(Vec::new());
    for n in nodes {
        write_node(&mut w, n);
    }
    w.into_inner()
}

fn write_node(w: &mut Writer<Vec<u8>>, node: &Node) {
    match node {
        Node::Elem(e) if e.self_closing && e.children.is_empty() => {
            let _ = w.write_event(Event::Empty(e.start.borrow()));
        }
        Node::Elem(e) => {
            let _ = w.write_event(Event::Start(e.start.borrow()));
            for c in &e.children {
                write_node(w, c);
            }
            let _ = w.write_event(Event::End(e.start.to_end()));
        }
        Node::Leaf(ev) => {
            let _ = w.write_event(ev.clone());
        }
    }
}

/// `true` when a leaf is a text node consisting only of XML whitespace — the
/// insignificant inter-element indentation the repair pass may drop when it
/// reorders a container's children.
pub(super) fn is_whitespace_text(node: &Node) -> bool {
    matches!(node, Node::Leaf(Event::Text(t)) if t.iter().all(u8::is_ascii_whitespace))
}
