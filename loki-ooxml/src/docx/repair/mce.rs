// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Markup-Compatibility repair: strip **undeclared** namespace prefixes from
//! `mc:Ignorable`.
//!
//! Every prefix listed in an `mc:Ignorable` attribute must resolve to an
//! in-scope namespace declaration (ISO/IEC 29500-3 §10.1.1 "Understanding
//! Namespaces"). A prefix that is never declared is a fatal error in Microsoft
//! Word — the classic "Word found unreadable content" repair prompt — even
//! though tolerant readers (Loki, `LibreOffice`) simply ignore the whole
//! Markup-Compatibility layer and open the file. Word's own output always pairs
//! `mc:Ignorable="w14 …"` with a matching `xmlns:w14="…"` on the same element.
//!
//! The fix is **lossless**: an undeclared prefix cannot bind any element or
//! attribute anywhere in scope, so removing it from `mc:Ignorable` changes
//! nothing a consumer could ever have processed. When stripping empties the
//! attribute, the attribute itself is dropped. Only the `mc:Ignorable` value is
//! rewritten; every other attribute is preserved byte-for-byte.

use quick_xml::events::BytesStart;

use super::RepairFinding;
use super::dom::{Elem, Node};

/// Walks the tree fixing `mc:Ignorable` on every element, threading the set of
/// namespace prefixes declared by ancestor elements so nested scopes resolve
/// correctly (in practice `mc:Ignorable` sits on a part root, but tracking the
/// scope keeps the check correct wherever it appears).
pub(super) fn fix_ignorable_tree(
    nodes: &mut [Node],
    part: &str,
    apply: bool,
    findings: &mut Vec<RepairFinding>,
) {
    walk(nodes, &[], part, apply, findings);
}

fn walk(
    nodes: &mut [Node],
    ancestor_prefixes: &[String],
    part: &str,
    apply: bool,
    findings: &mut Vec<RepairFinding>,
) {
    for node in nodes.iter_mut() {
        if let Node::Elem(e) = node {
            // Prefixes declared on this element extend the ancestor scope.
            let mut scope = ancestor_prefixes.to_vec();
            scope.extend(declared_prefixes(&e.start));
            fix_ignorable(e, &scope, part, apply, findings);
            walk(&mut e.children, &scope, part, apply, findings);
        }
    }
}

/// The namespace prefixes declared by `xmlns:PREFIX="…"` attributes on `start`
/// (the default `xmlns="…"` declaration has no prefix and is skipped).
fn declared_prefixes(start: &BytesStart<'_>) -> Vec<String> {
    start
        .attributes()
        .flatten()
        .filter_map(|a| {
            a.key
                .as_ref()
                .strip_prefix(b"xmlns:")
                .map(|p| String::from_utf8_lossy(p).into_owned())
        })
        .collect()
}

/// The raw `mc:Ignorable` attribute value on `start`, if present.
fn ignorable_value(start: &BytesStart<'_>) -> Option<String> {
    start.attributes().flatten().find_map(|a| {
        (a.key.as_ref() == b"mc:Ignorable").then(|| String::from_utf8_lossy(&a.value).into_owned())
    })
}

/// Detects (and, when `apply`, removes) undeclared prefixes in one element's
/// `mc:Ignorable`.
fn fix_ignorable(
    e: &mut Elem,
    scope: &[String],
    part: &str,
    apply: bool,
    findings: &mut Vec<RepairFinding>,
) {
    let Some(value) = ignorable_value(&e.start) else {
        return;
    };
    let (kept, dropped): (Vec<&str>, Vec<&str>) = value
        .split_whitespace()
        .partition(|p| scope.iter().any(|d| d == p));
    if dropped.is_empty() {
        return; // every listed prefix resolves — nothing to fix
    }

    findings.push(RepairFinding {
        part: part.to_string(),
        container: "mc:Ignorable".to_string(),
        detail: format!(
            "mc:Ignorable lists undeclared namespace prefix(es) ({}); Word \
             rejects this as unreadable content — removing the unresolvable prefix(es)",
            dropped.join(", ")
        ),
    });

    if !apply {
        return;
    }
    rewrite_ignorable(&mut e.start, &kept.join(" "));
}

/// Rewrites the `mc:Ignorable` value in `start` to `new_value` (or drops the
/// whole attribute when `new_value` is empty), leaving every other byte of the
/// start tag untouched. Byte-surgery avoids re-escaping the surrounding
/// attributes the way a rebuild via `push_attribute` would.
fn rewrite_ignorable(start: &mut BytesStart<'static>, new_value: &str) {
    let name_len = start.name().as_ref().len();
    let buf = start.to_vec();
    let Some(span) = ignorable_span(&buf) else {
        return;
    };

    let mut out = Vec::with_capacity(buf.len());
    if new_value.is_empty() {
        // Drop the attribute together with one run of leading whitespace so the
        // tag does not keep a doubled or trailing separator.
        let mut ws = span.key_start;
        while ws > 0 && buf[ws - 1].is_ascii_whitespace() {
            ws -= 1;
        }
        out.extend_from_slice(&buf[..ws]);
        out.extend_from_slice(&buf[span.attr_end..]);
    } else {
        out.extend_from_slice(&buf[..span.value_start]);
        out.extend_from_slice(new_value.as_bytes());
        out.extend_from_slice(&buf[span.value_end..]);
    }
    let content = String::from_utf8_lossy(&out).into_owned();
    *start = BytesStart::from_content(content, name_len);
}

/// Byte offsets of the `mc:Ignorable="…"` attribute inside a start-tag buffer.
struct IgnorableSpan {
    /// Offset of the `m` in `mc:Ignorable`.
    key_start: usize,
    /// Offset just past the opening quote (first byte of the value).
    value_start: usize,
    /// Offset of the closing quote (one past the last value byte).
    value_end: usize,
    /// Offset just past the closing quote (end of the whole attribute).
    attr_end: usize,
}

/// Locates the `mc:Ignorable` attribute in a raw start-tag buffer. Requires the
/// key to begin at an attribute boundary (preceded by whitespace) so a stray
/// occurrence inside another attribute's value can never match.
fn ignorable_span(buf: &[u8]) -> Option<IgnorableSpan> {
    const KEY: &[u8] = b"mc:Ignorable";
    let key_start = (0..buf.len())
        .find(|&i| buf[i..].starts_with(KEY) && i > 0 && buf[i - 1].is_ascii_whitespace())?;

    let mut i = key_start + KEY.len();
    while i < buf.len() && buf[i] != b'=' {
        i += 1;
    }
    i += 1; // step past '='
    while i < buf.len() && buf[i].is_ascii_whitespace() {
        i += 1;
    }
    let quote = *buf.get(i)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let value_start = i + 1;
    let value_end = (value_start..buf.len()).find(|&j| buf[j] == quote)?;
    Some(IgnorableSpan {
        key_start,
        value_start,
        value_end,
        attr_end: value_end + 1,
    })
}
