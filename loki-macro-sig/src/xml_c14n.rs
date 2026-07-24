// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Canonical XML 1.0 (inclusive, without comments) for the ODF `XMLDSig` verifier
//! (8A.4; W3C `REC-xml-c14n-20010315`).
//!
//! W3C `XMLDSig` signs the **canonical** octet stream of `<SignedInfo>`, not its
//! serialised bytes, so verifying an ODF `macrosignatures.xml` signature means
//! re-canonicalising `SignedInfo` exactly as the signer did. This module does
//! that over a small namespace-resolved node tree ([`C14nElement`]) built by the
//! parser (8A.4b).
//!
//! **Scope.** This is the *inclusive* c14n the ODF/`LibreOffice` signer uses, for
//! the shapes `macrosignatures.xml` actually contains: elements, text,
//! attributes, and namespace declarations — no comments (`XMLDSig`'s default
//! omits them), no processing instructions, no DTD, no external entities. Those
//! constructs never appear inside a `SignedInfo`; a document that smuggled one in
//! would fail digest/signature comparison, never mis-verify. Line endings are
//! assumed already normalised to `#xA` by the XML reader.
//!
//! Total: it only ever transforms an in-memory tree to bytes; it cannot panic.

/// A qualified name as written: an optional prefix (`None` = default namespace /
/// unprefixed) and a local part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QName {
    /// The namespace prefix, or `None` for the default namespace / no prefix.
    pub prefix: Option<String>,
    /// The local name.
    pub local: String,
}

/// An attribute: its qualified name and its (already XML-unescaped) value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct C14nAttr {
    /// The attribute's qualified name.
    pub name: QName,
    /// The attribute value, as characters (canonicalisation re-escapes it).
    pub value: String,
}

/// One element in the tree to canonicalise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct C14nElement {
    /// The element's qualified name.
    pub name: QName,
    /// Namespace declarations *in scope* at this element as `(prefix, uri)` with
    /// `prefix == None` for the default namespace. The parser flattens every
    /// inherited namespace onto the apex `SignedInfo` (inclusive c14n renders all
    /// in-scope namespaces at the top of the node-set), so the tree is
    /// self-contained.
    pub ns_decls: Vec<(Option<String>, String)>,
    /// Non-namespace attributes.
    pub attrs: Vec<C14nAttr>,
    /// Child nodes in document order.
    pub children: Vec<C14nNode>,
}

/// A child node: a nested element or a run of character data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum C14nNode {
    /// A child element.
    Element(C14nElement),
    /// Character data (already XML-unescaped).
    Text(String),
}

const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// Canonicalises `root` and all descendants to the Canonical XML 1.0 octet
/// stream signed by `XMLDSig`.
pub(crate) fn canonicalize(root: &C14nElement) -> Vec<u8> {
    let mut out = String::new();
    write_element(root, &[], &mut out);
    out.into_bytes()
}

/// Writes one element, given the `(prefix, uri)` namespace nodes already rendered
/// by output ancestors, then recurses.
fn write_element(el: &C14nElement, rendered: &[(Option<String>, String)], out: &mut String) {
    let qname = qname_str(&el.name);
    out.push('<');
    out.push_str(&qname);

    // Namespace axis: render each in-scope declaration not already rendered
    // identically by an output ancestor (a superfluous xmlns="" is dropped).
    let mut to_emit: Vec<&(Option<String>, String)> = el
        .ns_decls
        .iter()
        .filter(|(prefix, uri)| {
            if prefix.is_none() && uri.is_empty() && !ancestor_has_default(rendered) {
                return false;
            }
            !rendered.iter().any(|(rp, ru)| rp == prefix && ru == uri)
        })
        .collect();
    // Default namespace first, then by prefix (the namespace node's local name).
    to_emit.sort_by_key(|ns| ns_sort_key(ns));
    for (prefix, uri) in &to_emit {
        out.push_str(" xmlns");
        if let Some(p) = prefix {
            out.push(':');
            out.push_str(p);
        }
        out.push_str("=\"");
        out.push_str(&escape_attr(uri));
        out.push('"');
    }

    // The scope for resolving attribute namespaces is the ancestors' plus ours.
    let mut scope: Vec<(Option<String>, String)> = rendered.to_vec();
    for decl in &el.ns_decls {
        scope.retain(|(p, _)| p != &decl.0);
        scope.push(decl.clone());
    }

    // Attributes sorted by (namespace URI, local name); unprefixed attrs have no
    // namespace and sort before any prefixed ones.
    let mut attrs: Vec<&C14nAttr> = el.attrs.iter().collect();
    attrs.sort_by(|a, b| {
        attr_ns_uri(&a.name, &scope)
            .cmp(&attr_ns_uri(&b.name, &scope))
            .then_with(|| a.name.local.cmp(&b.name.local))
    });
    for a in attrs {
        out.push(' ');
        out.push_str(&qname_str(&a.name));
        out.push_str("=\"");
        out.push_str(&escape_attr(&a.value));
        out.push('"');
    }

    out.push('>');
    // The rendered set handed to children is the ancestors' set updated with ours.
    let mut child_rendered: Vec<(Option<String>, String)> = rendered.to_vec();
    for decl in &el.ns_decls {
        if decl.0.is_some() || !decl.1.is_empty() {
            child_rendered.retain(|(p, _)| p != &decl.0);
            child_rendered.push(decl.clone());
        }
    }
    for child in &el.children {
        match child {
            C14nNode::Element(c) => write_element(c, &child_rendered, out),
            C14nNode::Text(t) => out.push_str(&escape_text(t)),
        }
    }
    out.push_str("</");
    out.push_str(&qname);
    out.push('>');
}

/// `prefix:local` or bare `local`.
fn qname_str(name: &QName) -> String {
    match &name.prefix {
        Some(p) => format!("{p}:{}", name.local),
        None => name.local.clone(),
    }
}

/// Whether any rendered ancestor namespace node is a (non-empty) default one, so
/// a local `xmlns=""` would be meaningful rather than superfluous.
fn ancestor_has_default(rendered: &[(Option<String>, String)]) -> bool {
    rendered
        .iter()
        .any(|(p, uri)| p.is_none() && !uri.is_empty())
}

/// Sort key for a namespace node: default (`xmlns`) first, then by prefix.
fn ns_sort_key(ns: &(Option<String>, String)) -> (u8, String) {
    match &ns.0 {
        None => (0, String::new()),
        Some(p) => (1, p.clone()),
    }
}

/// The namespace URI of an attribute for c14n sorting: unprefixed attributes are
/// in *no* namespace (empty URI); `xml:` is the reserved XML namespace; any other
/// prefix resolves through the in-scope declarations.
fn attr_ns_uri(name: &QName, scope: &[(Option<String>, String)]) -> String {
    match &name.prefix {
        None => String::new(),
        Some(p) if p == "xml" => XML_NS.to_owned(),
        Some(p) => scope
            .iter()
            .find(|(sp, _)| sp.as_deref() == Some(p))
            .map(|(_, uri)| uri.clone())
            .unwrap_or_default(),
    }
}

/// Canonical XML attribute-value escaping (`&`, `<`, `"`, and the tab/CR/LF
/// whitespace, but not `>`).
fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#x9;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            _ => out.push(c),
        }
    }
    out
}

/// Canonical XML text escaping (`&`, `<`, `>`, and CR; tabs and newlines are kept
/// literally).
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#xD;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "xml_c14n_tests.rs"]
mod tests;
