// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level DOCX package assembly.
//!
//! Assembles the OPC package from the serialized parts produced by the other
//! modules in `write/`, wires up relationships and content types, and writes
//! the ZIP to the caller-supplied writer.
//!
//! ADR-0007: export via `loki-opc`'s `Package::write`.

use std::io::{Seek, Write};

use loki_doc_model::document::Document;
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::document::{write_document_xml, write_header_footer_xml};
use crate::docx::write::footnotes::{write_endnotes_xml, write_footnotes_xml};
use crate::docx::write::numbering::write_numbering_xml;
use crate::docx::write::rels::{AuxParts, add_document_relationships};
use crate::docx::write::styles::write_styles_xml;
use crate::error::OoxmlError;

// ── OPC relationship type URIs ───────────────────────────────────────────────

const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";

// ── OOXML media types ────────────────────────────────────────────────────────

const MT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
/// Content type for the main part of a Word **template** (`.dotx`). Structurally
/// identical to a `.docx`; only this override differs.
const MT_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml";

/// Whether to assemble a regular document (`.docx`) or a template (`.dotx`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DocxKind {
    /// A normal document part (`document.main+xml`).
    Document,
    /// A template part (`template.main+xml`).
    Template,
}

impl DocxKind {
    /// The main-part content type for this kind.
    fn main_content_type(self) -> &'static str {
        match self {
            DocxKind::Document => MT_DOCUMENT,
            DocxKind::Template => MT_TEMPLATE,
        }
    }
}
const MT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const MT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const MT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
const MT_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
const MT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const MT_FOOTER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
const MT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";

/// Assembles a complete `.docx` package from `doc` and writes it to `writer`.
pub(crate) fn assemble_docx(doc: &Document, writer: impl Write + Seek) -> Result<(), OoxmlError> {
    assemble_docx_kind(doc, writer, DocxKind::Document)
}

/// Assembles a `.docx` or `.dotx` package, depending on `kind`, and writes it.
#[allow(clippy::too_many_lines)] // Pre-existing pattern — structural refactor deferred
pub(crate) fn assemble_docx_kind(
    doc: &Document,
    writer: impl Write + Seek,
    kind: DocxKind,
) -> Result<(), OoxmlError> {
    // ── Step 1: Build styles.xml ─────────────────────────────────────────
    let styles_bytes = write_styles_xml(&doc.styles);

    // ── Step 2: Build document.xml (and collect all document-linked state) ──
    let mut collector = ExportCollector::new();
    let document_bytes = write_document_xml(&doc.sections, &doc.styles, &mut collector);

    // ── Step 3: Build auxiliary parts ───────────────────────────────────
    let has_numbering = !collector.num_state.is_empty();
    let has_footnotes = !collector.footnotes.is_empty();
    let has_endnotes = !collector.endnotes.is_empty();

    let numbering_bytes = if has_numbering {
        Some(write_numbering_xml(&collector.num_state))
    } else {
        None
    };

    let footnotes_bytes = if has_footnotes {
        Some(write_footnotes_xml(&mut collector))
    } else {
        None
    };

    let endnotes_bytes = if has_endnotes {
        Some(write_endnotes_xml(&mut collector))
    } else {
        None
    };

    // Even-page headers/footers only round-trip when the document declares
    // `w:evenAndOddHeaders` in settings.xml. Write that part when any section
    // carries an even-page variant.
    let needs_even_odd = doc
        .sections
        .iter()
        .any(|s| s.layout.header_even.is_some() || s.layout.footer_even.is_some());

    let mirror_margins = doc.settings.as_ref().is_some_and(|s| s.mirror_margins);
    let has_comments = !doc.comments.is_empty();
    let comments_bytes =
        has_comments.then(|| crate::docx::write::comments::write_comments_xml(&doc.comments));

    // ── Step 4: Assemble OPC package ─────────────────────────────────────
    let mut pkg = Package::new();

    // Part names.
    let doc_part = PartName::new("/word/document.xml").map_err(OoxmlError::Opc)?;
    let styles_part = PartName::new("/word/styles.xml").map_err(OoxmlError::Opc)?;
    let numbering_part = PartName::new("/word/numbering.xml").map_err(OoxmlError::Opc)?;
    let footnotes_part = PartName::new("/word/footnotes.xml").map_err(OoxmlError::Opc)?;
    let endnotes_part = PartName::new("/word/endnotes.xml").map_err(OoxmlError::Opc)?;
    let comments_part = PartName::new("/word/comments.xml").map_err(OoxmlError::Opc)?;

    // Insert parts.
    pkg.set_part(doc_part.clone(), PartData::new(document_bytes, MT_DOCUMENT));
    pkg.set_part(styles_part.clone(), PartData::new(styles_bytes, MT_STYLES));
    if let Some(nb) = numbering_bytes {
        pkg.set_part(numbering_part.clone(), PartData::new(nb, MT_NUMBERING));
    }
    if let Some(fb) = footnotes_bytes {
        pkg.set_part(footnotes_part.clone(), PartData::new(fb, MT_FOOTNOTES));
    }
    if let Some(eb) = endnotes_bytes {
        pkg.set_part(endnotes_part.clone(), PartData::new(eb, MT_ENDNOTES));
    }
    if let Some(cb) = comments_bytes {
        pkg.set_part(comments_part.clone(), PartData::new(cb, MT_COMMENTS));
    }

    // Insert headers and footers.
    let headers_footers = collector.take_headers_footers();
    for hf in &headers_footers {
        let hf_part = PartName::new(format!("/{}", hf.path)).map_err(OoxmlError::Opc)?;
        let mime = if hf.is_header { MT_HEADER } else { MT_FOOTER };
        let bytes = write_header_footer_xml(&hf.blocks, &mut collector, hf.is_header);
        pkg.set_part(hf_part, PartData::new(bytes, mime));
    }

    // Insert media parts.
    for m in &collector.media {
        let m_part = PartName::new(format!("/{}", m.path)).map_err(OoxmlError::Opc)?;
        let mime = match m.ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "application/octet-stream",
        };
        pkg.set_part(m_part, PartData::new(m.bytes.clone(), mime));
    }

    // ── Package-level relationship: /_rels/.rels → word/document.xml ─────
    pkg.relationships_mut()
        .add(Relationship {
            id: "rId1".to_string(),
            rel_type: REL_OFFICE_DOCUMENT.to_string(),
            target: "word/document.xml".to_string(),
            target_mode: TargetMode::Internal,
        })
        .map_err(OoxmlError::Opc)?;

    // ── Document-level relationships: word/_rels/document.xml.rels ───────
    add_document_relationships(
        &mut pkg,
        &doc_part,
        &mut collector,
        &headers_footers,
        AuxParts {
            numbering: has_numbering,
            footnotes: has_footnotes,
            endnotes: has_endnotes,
            even_odd: needs_even_odd,
            mirror_margins,
            comments: has_comments,
        },
    )?;

    // ── Document metadata ─────────────────────────────────────────────────
    // Core properties (docProps/core.xml) are serialized by the OPC layer;
    // the extended Dublin Core fields go to docProps/custom.xml.
    crate::docx::write::metadata::populate_core_properties(&mut pkg, &doc.meta);
    crate::docx::write::custom_props::add_custom_properties(&mut pkg, &doc.meta.dublin_core)?;

    // ── Content types ─────────────────────────────────────────────────────
    let ct = pkg.content_type_map_mut();
    ct.add_default(
        "rels",
        "application/vnd.openxmlformats-package.relationships+xml",
    );
    ct.add_default("xml", "application/xml");
    ct.add_override(&doc_part, kind.main_content_type());
    ct.add_override(&styles_part, MT_STYLES);
    if has_numbering {
        ct.add_override(&numbering_part, MT_NUMBERING);
    }
    if has_footnotes {
        ct.add_override(&footnotes_part, MT_FOOTNOTES);
    }
    if has_endnotes {
        ct.add_override(&endnotes_part, MT_ENDNOTES);
    }
    if has_comments {
        ct.add_override(&comments_part, MT_COMMENTS);
    }

    // Header/footer content types.
    for hf in &headers_footers {
        let hf_part = PartName::new(format!("/{}", hf.path)).map_err(OoxmlError::Opc)?;
        let mime = if hf.is_header { MT_HEADER } else { MT_FOOTER };
        ct.add_override(&hf_part, mime);
    }

    // Image content types.
    for m in &collector.media {
        let ext = &m.ext;
        let mime = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => continue,
        };
        ct.add_default(ext, mime);
    }

    // ── Step 5: Canonicalise child order, then write ZIP ──────────────────
    // The per-part serialisers emit content correctly but do not all emit
    // `pPr`/`rPr`/… children in the strict `xsd:sequence` order that
    // schema-validating consumers (Microsoft Word) require to open the file —
    // tolerant readers (Loki, LibreOffice) accept any order. This pass reorders
    // them so a document Loki writes opens in Word. It is semantics-preserving
    // (only element order changes), so it does not affect re-import. See
    // `docx::repair`.
    crate::docx::repair::canonicalize_package(&mut pkg);
    pkg.write(writer).map_err(OoxmlError::Opc)
}
