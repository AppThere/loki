// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Structural parse of an ODF `META-INF/macrosignatures.xml` (8A.4).
//!
//! ODF signs macros with W3C `XMLDSig`: a `document-signatures` root holds one or
//! more `Signature` elements, each a `SignedInfo` (the `SignatureMethod` plus a
//! `Reference` per signed package part), a `SignatureValue`, and a `KeyInfo`
//! carrying the signer `X509Certificate`. This module walks the
//! [`crate::odf_dom`] tree and pulls out exactly those pieces — **structure
//! only**: it resolves no algorithms and verifies nothing (that is
//! [`crate::verify_odf`]), so a signature with an unknown algorithm still parses
//! and later reads as `UnsupportedAlgorithm` rather than silently vanishing.
//!
//! The extracted `SignedInfo` has every in-scope namespace flattened onto its
//! apex, because inclusive Canonical XML renders all in-scope namespaces at the
//! top of the node-set — so [`crate::xml_c14n::canonicalize`] can run on it
//! standalone. Total on hostile input: malformed bytes yield an empty `Vec`,
//! decode failures leave empty byte fields that fail closed downstream (T9).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::odf_dom::parse_dom;
use crate::xml_c14n::{C14nElement, C14nNode};

/// One `Reference` inside a `SignedInfo`: the signed part's URI, its
/// `DigestMethod` algorithm URI, and the decoded `DigestValue`.
pub(crate) struct OdfReference {
    /// The `URI` attribute — a package part (`Basic/Standard/Module1.xml`) or an
    /// in-document fragment (`#...`).
    pub uri: String,
    /// The `DigestMethod` `Algorithm` URI (resolved to a hash by the verifier).
    pub digest_alg_uri: String,
    /// The decoded `DigestValue` (empty if the text was not valid base64).
    pub digest_value: Vec<u8>,
}

/// One parsed `Signature`: its canonicalisable `SignedInfo`, the signature
/// algorithm URI, references, and the decoded signature + signer certificate.
pub(crate) struct OdfSignature {
    /// `SignedInfo`, in-scope namespaces flattened onto the apex.
    pub signed_info: C14nElement,
    /// The `SignatureMethod` `Algorithm` URI (resolved by the verifier).
    pub signature_method_uri: String,
    /// The signed references.
    pub references: Vec<OdfReference>,
    /// The decoded `SignatureValue` (empty if the text was not valid base64).
    pub signature_value: Vec<u8>,
    /// The decoded signer `X509Certificate` DER (empty if absent / not base64).
    pub cert_der: Vec<u8>,
}

/// Parses every `Signature` in a `macrosignatures.xml`, in document order
/// (`XMLDSig` has no variant ranking, unlike the VBA streams). Returns empty for
/// non-XML input or a document with no signatures.
#[must_use]
pub(crate) fn parse_macro_signatures(xml: &[u8]) -> Vec<OdfSignature> {
    let Some(root) = parse_dom(xml) else {
        return Vec::new();
    };
    child_elements(&root, "Signature")
        .into_iter()
        .filter_map(|sig| parse_one(&root, sig))
        .collect()
}

/// Extracts one `OdfSignature` from a `Signature` element, given its `root` for
/// namespace-scope flattening. `None` if the mandatory `SignedInfo` is absent.
fn parse_one(root: &C14nElement, sig: &C14nElement) -> Option<OdfSignature> {
    let signed_info_el = first_child(sig, "SignedInfo")?;
    // Flatten root -> Signature -> SignedInfo namespace scope onto the apex.
    let scope = flatten_scope(&[root, sig, signed_info_el]);
    let mut signed_info = signed_info_el.clone();
    signed_info.ns_decls = scope;

    let signature_method_uri = first_child(signed_info_el, "SignatureMethod")
        .and_then(|m| attr(m, "Algorithm"))
        .unwrap_or_default()
        .to_owned();

    let references = child_elements(signed_info_el, "Reference")
        .into_iter()
        .map(parse_reference)
        .collect();

    let signature_value = first_child(sig, "SignatureValue")
        .map(text_content)
        .and_then(|t| decode_b64(&t))
        .unwrap_or_default();

    let cert_der = descendant(sig, "X509Certificate")
        .map(text_content)
        .and_then(|t| decode_b64(&t))
        .unwrap_or_default();

    Some(OdfSignature {
        signed_info,
        signature_method_uri,
        references,
        signature_value,
        cert_der,
    })
}

/// Builds an [`OdfReference`] from a `Reference` element.
fn parse_reference(r: &C14nElement) -> OdfReference {
    let uri = attr(r, "URI").unwrap_or_default().to_owned();
    let digest_alg_uri = first_child(r, "DigestMethod")
        .and_then(|m| attr(m, "Algorithm"))
        .unwrap_or_default()
        .to_owned();
    let digest_value = first_child(r, "DigestValue")
        .map(text_content)
        .and_then(|t| decode_b64(&t))
        .unwrap_or_default();
    OdfReference {
        uri,
        digest_alg_uri,
        digest_value,
    }
}

/// Merges the namespace declarations of `frames` (outermost first) into the final
/// in-scope `(prefix, uri)` set, dropping any default-namespace undeclaration.
fn flatten_scope(frames: &[&C14nElement]) -> Vec<(Option<String>, String)> {
    let mut scope: Vec<(Option<String>, String)> = Vec::new();
    for frame in frames {
        for (prefix, uri) in &frame.ns_decls {
            scope.retain(|(p, _)| p != prefix);
            if !(prefix.is_none() && uri.is_empty()) {
                scope.push((prefix.clone(), uri.clone()));
            }
        }
    }
    scope
}

/// Direct child elements whose local name matches (prefix-insensitive).
fn child_elements<'a>(el: &'a C14nElement, local: &str) -> Vec<&'a C14nElement> {
    el.children
        .iter()
        .filter_map(|n| match n {
            C14nNode::Element(c) if c.name.local == local => Some(c),
            _ => None,
        })
        .collect()
}

/// First direct child element with the given local name.
fn first_child<'a>(el: &'a C14nElement, local: &str) -> Option<&'a C14nElement> {
    child_elements(el, local).into_iter().next()
}

/// First descendant element with the given local name (depth-first).
fn descendant<'a>(el: &'a C14nElement, local: &str) -> Option<&'a C14nElement> {
    for child in &el.children {
        if let C14nNode::Element(c) = child {
            if c.name.local == local {
                return Some(c);
            }
            if let Some(found) = descendant(c, local) {
                return Some(found);
            }
        }
    }
    None
}

/// The value of a non-namespace attribute with the given local name.
fn attr<'a>(el: &'a C14nElement, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|a| a.name.prefix.is_none() && a.name.local == local)
        .map(|a| a.value.as_str())
}

/// Concatenated text of all descendant text nodes.
fn text_content(el: &C14nElement) -> String {
    let mut out = String::new();
    collect_text(el, &mut out);
    out
}

fn collect_text(el: &C14nElement, out: &mut String) {
    for child in &el.children {
        match child {
            C14nNode::Text(t) => out.push_str(t),
            C14nNode::Element(c) => collect_text(c, out),
        }
    }
}

/// Decodes standard base64 after stripping ASCII whitespace (`XMLDSig` wraps long
/// values across lines). `None` on any decode error.
fn decode_b64(text: &str) -> Option<Vec<u8>> {
    let cleaned: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if cleaned.is_empty() {
        return None;
    }
    STANDARD.decode(cleaned).ok()
}

#[cfg(test)]
#[path = "odf_tests.rs"]
mod tests;
