// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX import entry point.
//!
//! [`DocxImport`] implements [`loki_doc_model::io::DocumentImport`] and is
//! the primary public API for converting a DOCX file into a
//! [`loki_doc_model::Document`].
//!
//! # Example
//!
//! ```no_run
//! use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
//! use loki_doc_model::io::DocumentImport;
//! let file = std::fs::File::open("document.docx").unwrap();
//! let doc = DocxImport::import(file, DocxImportOptions::default()).unwrap();
//! ```

use std::collections::HashMap;
use std::io::{Read, Seek};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_opc::{Package, PartData, PartName};

use crate::constants::{
    REL_COMMENTS, REL_ENDNOTES, REL_FOOTER, REL_FOOTNOTES, REL_HEADER, REL_HYPERLINK, REL_IMAGE,
    REL_NUMBERING, REL_OFFICE_DOCUMENT, REL_SETTINGS, REL_STYLES,
};
use crate::docx::mapper::document::map_document;
use crate::docx::model::paragraph::DocxParagraph;
use crate::docx::reader::document::parse_document;
use crate::docx::reader::footnotes::parse_notes;
use crate::docx::reader::header_footer::parse_header_footer;
use crate::docx::reader::numbering::parse_numbering;
use crate::docx::reader::settings::parse_settings;
use crate::docx::reader::styles::parse_styles;
use crate::error::{OoxmlError, OoxmlResult, OoxmlWarning};

#[path = "import_pic_bullets.rs"]
mod import_pic_bullets;

/// Options controlling DOCX import behaviour.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct DocxImportOptions {
    /// When `true` (default), paragraphs whose style name starts with
    /// `"heading"` map to a `Block::Heading` rather than a plain paragraph.
    pub emit_heading_blocks: bool,

    /// When `true` (default), images are embedded as data URIs
    /// (`data:<media-type>;base64,<data>`); when `false`, image parts are
    /// omitted from the output.
    pub embed_images: bool,
}

impl Default for DocxImportOptions {
    fn default() -> Self {
        Self {
            emit_heading_blocks: true,
            embed_images: true,
        }
    }
}

/// The result of a successful DOCX import.
#[derive(Debug)]
pub struct DocxImportResult {
    /// The imported document in the format-neutral abstract model.
    pub document: Document,

    /// Non-fatal issues encountered during import (unresolved relationships,
    /// unsupported features, etc.).
    pub warnings: Vec<OoxmlWarning>,
}

/// Unit struct that implements [`DocumentImport`] for DOCX files.
///
/// Construct import options with [`DocxImportOptions`] and call
/// [`DocumentImport::import`], or use [`DocxImporter`] directly for access
/// to the full [`DocxImportResult`] (including warnings).
pub struct DocxImport;

impl DocumentImport for DocxImport {
    type Error = OoxmlError;
    type Options = DocxImportOptions;

    /// Imports a DOCX file and returns the abstract document.
    ///
    /// Warnings are discarded. Use [`DocxImporter`] to retrieve them.
    fn import(reader: impl Read + Seek, options: Self::Options) -> Result<Document, Self::Error> {
        DocxImporter::new(options).run(reader).map(|r| r.document)
    }
}

/// Stateful DOCX importer that preserves [`OoxmlWarning`]s alongside the
/// imported [`Document`] — use it when you need the non-fatal import issues.
pub struct DocxImporter {
    options: DocxImportOptions,
}

impl DocxImporter {
    /// Creates a new importer with the given options.
    #[must_use]
    pub fn new(options: DocxImportOptions) -> Self {
        Self { options }
    }

    /// Opens the DOCX container and translates it into a [`DocxImportResult`]:
    /// open the OPC/ZIP package, locate the main `officeDocument` part, parse
    /// the body/styles/numbering/notes XML, collect hyperlink targets and
    /// (optionally) image bytes, then call `map_document`.
    ///
    /// # Errors
    ///
    /// Returns an error if the ZIP container is malformed, the required
    /// `officeDocument` relationship is missing, or a mandatory part fails to
    /// parse.
    pub fn run(self, reader: impl Read + Seek) -> OoxmlResult<DocxImportResult> {
        let package = Package::open(reader)?;
        let (document, warnings) = parse_and_map_package(&package, &self.options)?;
        Ok(DocxImportResult { document, warnings })
    }
}

// ── Shared parse-and-map pipeline ─────────────────────────────────────────────

/// Parses all DOCX parts from an open OPC [`Package`] and maps them to a
/// [`loki_doc_model::Document`].
///
/// Used by both [`DocxImporter::run`] and the public
/// [`crate::docx::mapper::map_document`] entry point.
// Function body is a single large match over XML events; splitting would reduce readability.
#[allow(clippy::too_many_lines)]
pub(crate) fn parse_and_map_package(
    package: &Package,
    options: &DocxImportOptions,
) -> OoxmlResult<(loki_doc_model::document::Document, Vec<OoxmlWarning>)> {
    // ── Locate the main document part ─────────────────────────────────
    let doc_rel = rels_by_type(package.relationships(), REL_OFFICE_DOCUMENT)
        .next()
        .ok_or_else(|| OoxmlError::MissingPart {
            relationship_type: REL_OFFICE_DOCUMENT.to_owned(),
        })?
        .clone();

    let doc_part_name = resolve_part_name("/", &doc_rel.target)?;

    let doc_bytes = package
        .part(&doc_part_name)
        .ok_or_else(|| OoxmlError::MissingPart {
            relationship_type: doc_part_name.as_str().to_owned(),
        })?
        .bytes
        .clone();

    // ── Parse main document ────────────────────────────────────────────
    let raw_doc = parse_document(&doc_bytes).map_err(|e| map_xml_err(e, doc_part_name.as_str()))?;

    // ── Parse optional related parts ──────────────────────────────────
    let doc_rels = package.part_relationships(&doc_part_name);

    let raw_styles = if let Some(rels) = doc_rels {
        if let Some(rel) = rels_by_type(rels, REL_STYLES).next() {
            let name = resolve_part_name(doc_part_name.as_str(), &rel.target)?;
            if let Some(part) = package.part(&name) {
                Some(parse_styles(&part.bytes).map_err(|e| map_xml_err(e, name.as_str()))?)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    let raw_styles = raw_styles.unwrap_or_default();

    let mut raw_numbering = resolve_optional_part(
        package,
        doc_rels,
        REL_NUMBERING,
        doc_part_name.as_str(),
        |bytes, _part| parse_numbering(bytes),
    )?;
    // Resolve picture-bullet images (via `word/numbering.xml.rels`) to data URIs
    // so they render (feature 5.4).
    import_pic_bullets::resolve(
        package,
        doc_rels,
        doc_part_name.as_str(),
        raw_numbering.as_mut(),
        options.embed_images,
    );

    let raw_footnotes = resolve_optional_part(
        package,
        doc_rels,
        REL_FOOTNOTES,
        doc_part_name.as_str(),
        parse_notes,
    )?;

    let raw_endnotes = resolve_optional_part(
        package,
        doc_rels,
        REL_ENDNOTES,
        doc_part_name.as_str(),
        parse_notes,
    )?;

    let raw_settings = resolve_optional_part(
        package,
        doc_rels,
        REL_SETTINGS,
        doc_part_name.as_str(),
        |bytes, _part| parse_settings(bytes),
    )?;

    let comments = resolve_optional_part(
        package,
        doc_rels,
        REL_COMMENTS,
        doc_part_name.as_str(),
        |bytes, _part| crate::docx::reader::comments::parse_comments(bytes),
    )?
    .unwrap_or_default();

    // ── Build hyperlinks and images maps ──────────────────────────────
    let mut hyperlinks: HashMap<String, String> = HashMap::new();
    let mut images: HashMap<String, PartData> = HashMap::new();
    let mut header_parts: HashMap<String, Vec<DocxParagraph>> = HashMap::new();
    let mut footer_parts: HashMap<String, Vec<DocxParagraph>> = HashMap::new();

    if let Some(rels) = doc_rels {
        for rel in rels_by_type(rels, REL_HYPERLINK) {
            hyperlinks.insert(rel.id.clone(), rel.target.clone());
        }

        if options.embed_images {
            for rel in rels_by_type(rels, REL_IMAGE) {
                if let Ok(img_name) = resolve_part_name(doc_part_name.as_str(), &rel.target)
                    && let Some(part) = package.part(&img_name)
                {
                    images.insert(rel.id.clone(), part.clone());
                }
            }
        }

        for rel in rels_by_type(rels, REL_HEADER) {
            if let Ok(name) = resolve_part_name(doc_part_name.as_str(), &rel.target)
                && let Some(part) = package.part(&name)
                && let Ok(paras) = parse_header_footer(&part.bytes, name.as_str())
            {
                header_parts.insert(rel.id.clone(), paras);
            }
        }

        for rel in rels_by_type(rels, REL_FOOTER) {
            if let Ok(name) = resolve_part_name(doc_part_name.as_str(), &rel.target)
                && let Some(part) = package.part(&name)
                && let Ok(paras) = parse_header_footer(&part.bytes, name.as_str())
            {
                footer_parts.insert(rel.id.clone(), paras);
            }
        }
    }

    // ── Map everything to the abstract model ──────────────────────────
    let (mut document, warnings) = map_document(
        &raw_doc,
        &raw_styles,
        raw_numbering.as_ref(),
        raw_footnotes.as_ref(),
        raw_endnotes.as_ref(),
        &images,
        &hyperlinks,
        &header_parts,
        &footer_parts,
        raw_settings.as_ref(),
        package.core_properties(),
        options,
    );

    // Extended Dublin Core from docProps/custom.xml (core.xml only covers the
    // core subset + dc:identifier).
    crate::docx::reader::custom_props::apply_extended_dc(package, &mut document.meta.dublin_core);

    // Comment bodies parsed from word/comments.xml (anchors are already in the
    // content flow as Inline::Comment).
    document.comments = comments;

    Ok((document, warnings))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Wraps an [`OoxmlError::Xml`] with the given part path for context.
fn map_xml_err(e: OoxmlError, _part: &str) -> OoxmlError {
    // The error already carries its part context from the reader; pass through.
    e
}

/// Resolves a target path relative to a base part name into a [`PartName`].
///
/// `base` should be a valid OPC part name (e.g. `"/word/document.xml"`).
/// If `target` starts with `/`, it is used as-is. Otherwise, the parent
/// directory of `base` is prepended.
fn resolve_part_name(base: &str, target: &str) -> OpcResult<PartName> {
    if target.starts_with('/') {
        return PartName::new(target).map_err(OoxmlError::Opc);
    }
    let dir = base.rfind('/').map_or("/", |i| &base[..=i]);
    PartName::new(format!("{dir}{target}")).map_err(OoxmlError::Opc)
}

/// Helper to retrieve relationships by type supporting both transitional and strict namespaces.
fn rels_by_type<'a>(
    rels: &'a loki_opc::RelationshipSet,
    transitional_type: &str,
) -> impl Iterator<Item = &'a loki_opc::Relationship> {
    let strict_type = transitional_type.replace(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/",
        "http://purl.oclc.org/ooxml/officeDocument/relationships/",
    );
    let strict_owned = strict_type;
    let trans_owned = transitional_type.to_owned();
    rels.iter()
        .filter(move |r| r.rel_type == trans_owned || r.rel_type == strict_owned)
}

type OpcResult<T> = Result<T, OoxmlError>;

/// Resolves an optional related part by relationship type and parses it.
///
/// Returns `None` if the relationship is not present; returns an error only
/// if the part exists but fails to parse.
fn resolve_optional_part<T, F>(
    package: &Package,
    doc_rels: Option<&loki_opc::RelationshipSet>,
    rel_type: &str,
    base_part: &str,
    parse: F,
) -> OpcResult<Option<T>>
where
    F: Fn(&[u8], &str) -> OpcResult<T>,
{
    let Some(rels) = doc_rels else {
        return Ok(None);
    };
    let Some(rel) = rels_by_type(rels, rel_type).next() else {
        return Ok(None);
    };
    let part_name = resolve_part_name(base_part, &rel.target)?;
    let Some(part) = package.part(&part_name) else {
        return Ok(None);
    };
    let result = parse(&part.bytes, part_name.as_str())?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_emit_heading_blocks() {
        assert!(DocxImportOptions::default().emit_heading_blocks);
    }

    #[test]
    fn default_options_embed_images() {
        assert!(DocxImportOptions::default().embed_images);
    }

    #[test]
    fn resolve_relative_path() {
        let name = resolve_part_name("/word/document.xml", "styles.xml").unwrap();
        assert_eq!(name.as_str(), "/word/styles.xml");
    }

    #[test]
    fn resolve_absolute_path() {
        let name = resolve_part_name("/word/document.xml", "/word/styles.xml").unwrap();
        assert_eq!(name.as_str(), "/word/styles.xml");
    }
}
