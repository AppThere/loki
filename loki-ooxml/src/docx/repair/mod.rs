// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX repair: detect and fix schema **child-ordering** violations that make a
//! document unreadable in Microsoft Word while a tolerant reader (Loki,
//! `LibreOffice`) opens it fine.
//!
//! OOXML complex types are `xsd:sequence`s, so Word rejects a file whose
//! `w:pPr`/`w:rPr`/`w:sectPr`/… children appear out of order (the classic
//! "Word found unreadable content" repair prompt). Hand-authored files and some
//! tool exporters emit these routinely. [`analyze_docx`] reports the
//! violations; [`repair_docx`] rewrites the offending parts into the required
//! order — a **lossless** transform that only reorders element children (never
//! touches attributes or text), so nothing the reader models is lost, and
//! constructs Loki itself cannot model survive untouched.
//!
//! Scope: the ordering axis (the dominant Word-vs-tolerant asymmetry). Other
//! corruption classes (dangling relationships, missing content types) are out
//! of scope here — the OPC layer already validates those on open.

use std::io::Cursor;

use loki_opc::Package;
use loki_opc::part::{PartData, PartName};

use crate::error::OoxmlError;

mod dom;
mod order;

use dom::{Elem, Node};

/// The position of a child `local` name in a schema order table, if present.
fn schema_index(order: &[&str], local: &str) -> Option<usize> {
    order.iter().position(|&n| n == local)
}

#[cfg(test)]
#[path = "repair_tests.rs"]
mod tests;

/// One schema-ordering violation found in a document part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairFinding {
    /// The OPC part path, e.g. `"word/document.xml"`.
    pub part: String,
    /// The container element whose children were out of order, e.g. `"w:pPr"`.
    pub container: String,
    /// Human-readable description of the violation and its fix.
    pub detail: String,
}

/// The outcome of analysing or repairing a document.
#[derive(Debug, Clone, Default)]
pub struct RepairReport {
    /// Every ordering violation found (in document order).
    pub findings: Vec<RepairFinding>,
    /// `true` if this report came from [`repair_docx`] (fixes applied) rather
    /// than [`analyze_docx`] (detection only).
    pub repaired: bool,
}

impl RepairReport {
    /// `true` when no violations were found.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Analyses `docx_bytes` for schema child-ordering violations **without**
/// modifying anything. Use the report to decide whether to offer a repair.
pub fn analyze_docx(docx_bytes: &[u8]) -> Result<RepairReport, OoxmlError> {
    let (_, report) = process(docx_bytes, false)?;
    Ok(report)
}

/// Repairs `docx_bytes`, returning the corrected package bytes and a report of
/// what was fixed. When the report [`is_clean`](RepairReport::is_clean), the
/// returned bytes are a faithful re-serialisation with no ordering changes.
pub fn repair_docx(docx_bytes: &[u8]) -> Result<(Vec<u8>, RepairReport), OoxmlError> {
    process(docx_bytes, true)
}

/// `true` for the `WordprocessingML` XML parts whose element order Word
/// enforces. The `DrawingML` theme part uses a different schema (its elements
/// never match our tables, but we skip it explicitly), and media/binary parts
/// are not XML.
// OPC part names for WordprocessingML are always lowercase `.xml` (§7.3), so a
// case-sensitive suffix test is correct here.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_wml_part(name: &str) -> bool {
    name.starts_with("/word/")
        && name.ends_with(".xml")
        && !name.starts_with("/word/theme/")
        && !name.starts_with("/word/media/")
}

/// Canonicalises the child order of every `WordprocessingML` part in an open
/// package, in place. Shared by [`repair_docx`] and the DOCX **export** path,
/// so files Loki writes open in schema-strict consumers (Word) regardless of
/// the order the serialisers happen to emit children.
pub(crate) fn canonicalize_package(pkg: &mut Package) -> RepairReport {
    let names: Vec<PartName> = pkg
        .part_names()
        .filter(|n| is_wml_part(n.as_str()))
        .cloned()
        .collect();

    let mut findings = Vec::new();
    for name in names {
        let Some(part) = pkg.part(&name) else {
            continue;
        };
        let (bytes, media_type) = (part.bytes.clone(), part.media_type.clone());
        let display = name.as_str().trim_start_matches('/').to_string();
        if let Some(repaired) = repair_part(&display, &bytes, true, &mut findings) {
            pkg.set_part(name, PartData::new(repaired, media_type));
        }
    }
    RepairReport {
        findings,
        repaired: true,
    }
}

/// Reorders one part's children (best effort). Returns the rewritten bytes when
/// `apply` and something changed, else `None`. A part that does not parse as XML
/// is left untouched — malformed XML is a different error class handled on
/// import, not by the ordering pass.
fn repair_part(
    display: &str,
    bytes: &[u8],
    apply: bool,
    findings: &mut Vec<RepairFinding>,
) -> Option<Vec<u8>> {
    let mut nodes = dom::parse(bytes).ok()?;
    let before = findings.len();
    reorder_tree(&mut nodes, display, apply, findings);
    (apply && findings.len() > before).then(|| dom::serialize(&nodes))
}

fn process(docx_bytes: &[u8], apply: bool) -> Result<(Vec<u8>, RepairReport), OoxmlError> {
    let mut pkg = Package::open(Cursor::new(docx_bytes)).map_err(OoxmlError::Opc)?;

    if !apply {
        // Analysis only: walk each WML part, collect findings, change nothing.
        let names: Vec<PartName> = pkg
            .part_names()
            .filter(|n| is_wml_part(n.as_str()))
            .cloned()
            .collect();
        let mut findings = Vec::new();
        for name in names {
            if let Some(part) = pkg.part(&name) {
                let display = name.as_str().trim_start_matches('/').to_string();
                let _ = repair_part(&display, &part.bytes.clone(), false, &mut findings);
            }
        }
        return Ok((
            docx_bytes.to_vec(),
            RepairReport {
                findings,
                repaired: false,
            },
        ));
    }

    let report = canonicalize_package(&mut pkg);
    let out = if report.is_clean() {
        docx_bytes.to_vec()
    } else {
        let mut buf = Cursor::new(Vec::new());
        pkg.write(&mut buf).map_err(OoxmlError::Opc)?;
        buf.into_inner()
    };
    Ok((out, report))
}

/// Depth-first walk: recurse into every element, then reorder the children of
/// any order-checked container.
fn reorder_tree(nodes: &mut [Node], part: &str, apply: bool, findings: &mut Vec<RepairFinding>) {
    for node in nodes.iter_mut() {
        if let Node::Elem(e) = node {
            reorder_tree(&mut e.children, part, apply, findings);
            if let Some(order) = order::schema_order(&e.local()) {
                fix_container(e, order, part, apply, findings);
            }
        }
    }
}

/// Checks (and optionally fixes) one container's child order.
fn fix_container(
    e: &mut Elem,
    order: &[&str],
    part: &str,
    apply: bool,
    findings: &mut Vec<RepairFinding>,
) {
    // Classify children. Bail out conservatively if the container holds anything
    // we can't safely position: a foreign element (e.g. an `mc:AlternateContent`
    // or `w14:*` extension) or significant text/comment content.
    let mut idxs = Vec::new();
    for c in &e.children {
        match c {
            Node::Elem(ce) => match schema_index(order, &ce.local()) {
                Some(i) => idxs.push(i),
                None => return, // foreign element — leave the whole container alone
            },
            Node::Leaf(_) if dom::is_whitespace_text(c) => {}
            Node::Leaf(_) => return, // comment / significant text — leave it alone
        }
    }
    if idxs.windows(2).all(|w| w[0] <= w[1]) {
        return; // already in schema order
    }

    let names: Vec<String> = e
        .children
        .iter()
        .filter_map(|c| match c {
            Node::Elem(ce) => Some(format!("w:{}", ce.local())),
            Node::Leaf(_) => None,
        })
        .collect();
    findings.push(RepairFinding {
        part: part.to_string(),
        container: format!("w:{}", e.local()),
        detail: format!(
            "child elements out of the required ECMA-376 order ({}); \
             Word rejects this — reorder to schema sequence",
            names.join(", ")
        ),
    });

    if !apply {
        return;
    }
    // Rebuild: keep only the element children, stable-sorted by schema index
    // (dropping the insignificant inter-element whitespace).
    let mut elems: Vec<Elem> = e
        .children
        .drain(..)
        .filter_map(|c| match c {
            Node::Elem(ce) => Some(ce),
            Node::Leaf(_) => None,
        })
        .collect();
    elems.sort_by_key(|ce| schema_index(order, &ce.local()).unwrap_or(usize::MAX));
    e.children = elems.into_iter().map(Node::Elem).collect();
}
