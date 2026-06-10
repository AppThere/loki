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
use crate::docx::write::styles::write_styles_xml;
use crate::docx::write::xml::{
    REL_ENDNOTES, REL_FOOTER, REL_FOOTNOTES, REL_HEADER, REL_IMAGE, REL_NUMBERING, REL_STYLES,
};
use crate::error::OoxmlError;

// ── OPC relationship type URIs ───────────────────────────────────────────────

const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";

// ── OOXML media types ────────────────────────────────────────────────────────

const MT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const MT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const MT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const MT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
const MT_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
const MT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const MT_FOOTER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";

/// Assembles a complete `.docx` package from `doc` and writes it to `writer`.
#[allow(clippy::too_many_lines)] // Pre-existing pattern — structural refactor deferred
pub(crate) fn assemble_docx(doc: &Document, writer: impl Write + Seek) -> Result<(), OoxmlError> {
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

    // ── Step 4: Assemble OPC package ─────────────────────────────────────
    let mut pkg = Package::new();

    // Part names.
    let doc_part = PartName::new("/word/document.xml").map_err(OoxmlError::Opc)?;
    let styles_part = PartName::new("/word/styles.xml").map_err(OoxmlError::Opc)?;
    let numbering_part = PartName::new("/word/numbering.xml").map_err(OoxmlError::Opc)?;
    let footnotes_part = PartName::new("/word/footnotes.xml").map_err(OoxmlError::Opc)?;
    let endnotes_part = PartName::new("/word/endnotes.xml").map_err(OoxmlError::Opc)?;

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
    pkg.part_relationships_mut(&doc_part)
        .add(Relationship {
            id: "rId1".to_string(),
            rel_type: REL_STYLES.to_string(),
            target: "styles.xml".to_string(),
            target_mode: TargetMode::Internal,
        })
        .map_err(OoxmlError::Opc)?;

    // numbering
    if has_numbering {
        pkg.part_relationships_mut(&doc_part)
            .add(Relationship {
                id: "rId2".to_string(),
                rel_type: REL_NUMBERING.to_string(),
                target: "numbering.xml".to_string(),
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;
    }

    // footnotes
    if has_footnotes {
        pkg.part_relationships_mut(&doc_part)
            .add(Relationship {
                id: "rId3".to_string(), // TODO: Should we use collector to manage these IDs too?
                rel_type: REL_FOOTNOTES.to_string(),
                target: "footnotes.xml".to_string(),
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;
    }

    // endnotes
    if has_endnotes {
        pkg.part_relationships_mut(&doc_part)
            .add(Relationship {
                id: "rId4".to_string(),
                rel_type: REL_ENDNOTES.to_string(),
                target: "endnotes.xml".to_string(),
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;
    }

    // hyperlinks
    for (r_id, url) in &collector.hyperlinks {
        pkg.part_relationships_mut(&doc_part)
            .add(Relationship {
                id: r_id.clone(),
                rel_type: crate::docx::write::xml::REL_HYPERLINK.to_string(),
                target: url.clone(),
                target_mode: TargetMode::External,
            })
            .map_err(OoxmlError::Opc)?;
    }

    // media
    for m in &collector.media {
        pkg.part_relationships_mut(&doc_part)
            .add(Relationship {
                id: m.r_id.clone(),
                rel_type: REL_IMAGE.to_string(),
                target: m.path.strip_prefix("word/").unwrap_or(&m.path).to_string(),
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;
    }

    // headers/footers
    for hf in &headers_footers {
        let rel_type = if hf.is_header { REL_HEADER } else { REL_FOOTER };
        pkg.part_relationships_mut(&doc_part)
            .add(Relationship {
                id: hf.r_id.clone(),
                rel_type: rel_type.to_string(),
                target: hf
                    .path
                    .strip_prefix("word/")
                    .unwrap_or(&hf.path)
                    .to_string(),
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;
    }

    // ── Content types ─────────────────────────────────────────────────────
    let ct = pkg.content_type_map_mut();
    ct.add_default(
        "rels",
        "application/vnd.openxmlformats-package.relationships+xml",
    );
    ct.add_default("xml", "application/xml");
    ct.add_override(&doc_part, MT_DOCUMENT);
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

    // ── Step 5: Write ZIP ─────────────────────────────────────────────────
    pkg.write(writer).map_err(OoxmlError::Opc)
}
