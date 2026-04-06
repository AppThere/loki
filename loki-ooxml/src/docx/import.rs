// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! DOCX import entry point.
//!
//! [`DocxImport`] implements [`loki_doc_model::io::DocumentImport`] and is
//! the primary public API for converting a DOCX file into a
//! [`loki_doc_model::Document`].
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
//! use loki_doc_model::io::DocumentImport;
//!
//! let file = File::open("document.docx").unwrap();
//! let doc = DocxImport::import(file, DocxImportOptions::default()).unwrap();
//! ```

use std::collections::HashMap;
use std::io::{Read, Seek};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_opc::{Package, PartData, PartName};

use crate::constants::{REL_ENDNOTES, REL_FOOTNOTES, REL_HYPERLINK, REL_IMAGE, REL_NUMBERING,
    REL_OFFICE_DOCUMENT, REL_SETTINGS, REL_STYLES};
use crate::docx::mapper::document::map_document;
use crate::docx::reader::document::parse_document;
use crate::docx::reader::footnotes::parse_notes;
use crate::docx::reader::numbering::parse_numbering;
use crate::docx::reader::settings::parse_settings;
use crate::docx::reader::styles::parse_styles;
use crate::error::{OoxmlError, OoxmlResult, OoxmlWarning};

/// Options controlling DOCX import behaviour.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct DocxImportOptions {
    /// When `true`, paragraphs whose style name starts with `"heading"` are
    /// mapped to [`loki_doc_model::content::block::Block::Heading`] rather
    /// than [`loki_doc_model::content::block::Block::Paragraph`].
    ///
    /// Defaults to `true`.
    pub emit_heading_blocks: bool,

    /// When `true`, images are embedded in the document as data URIs
    /// (`data:<media-type>;base64,<data>`). When `false`, image parts are
    /// omitted from the output.
    ///
    /// Defaults to `true`.
    pub embed_images: bool,
}

impl Default for DocxImportOptions {
    fn default() -> Self {
        Self { emit_heading_blocks: true, embed_images: true }
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
    fn import(
        reader: impl Read + Seek,
        options: Self::Options,
    ) -> Result<Document, Self::Error> {
        DocxImporter::new(options).run(reader).map(|r| r.document)
    }
}

/// Stateful DOCX importer that preserves [`OoxmlWarning`]s alongside the
/// imported [`Document`].
///
/// Use this type when you need to inspect non-fatal import issues.
pub struct DocxImporter {
    options: DocxImportOptions,
}

impl DocxImporter {
    /// Creates a new importer with the given options.
    #[must_use]
    pub fn new(options: DocxImportOptions) -> Self {
        Self { options }
    }

    /// Opens the DOCX container and translates it into a [`DocxImportResult`].
    ///
    /// Steps:
    /// 1. Open the OPC/ZIP package.
    /// 2. Locate the main `officeDocument` part via package relationships.
    /// 3. Parse XML for document body, styles, numbering, footnotes, endnotes.
    /// 4. Collect hyperlink targets and (optionally) image bytes.
    /// 5. Call `map_document` to produce the abstract model.
    ///
    /// # Errors
    ///
    /// Returns an error if the ZIP container is malformed, if the required
    /// `officeDocument` relationship is missing, or if any mandatory part
    /// cannot be parsed.
    pub fn run(self, reader: impl Read + Seek) -> OoxmlResult<DocxImportResult> {
        let package = Package::open(reader)?;

        // ── Locate the main document part ─────────────────────────────────
        let doc_rel = package
            .relationships()
            .by_type(REL_OFFICE_DOCUMENT)
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
        let raw_doc = parse_document(&doc_bytes)
            .map_err(|e| map_xml_err(e, doc_part_name.as_str()))?;

        // ── Parse optional related parts ──────────────────────────────────
        let doc_rels = package.part_relationships(&doc_part_name);

        let raw_styles = if let Some(rels) = doc_rels {
            if let Some(rel) = rels.by_type(REL_STYLES).next() {
                let name = resolve_part_name(doc_part_name.as_str(), &rel.target)?;
                if let Some(part) = package.part(&name) {
                    Some(
                        parse_styles(&part.bytes)
                            .map_err(|e| map_xml_err(e, name.as_str()))?,
                    )
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

        let raw_numbering = resolve_optional_part(
            &package,
            doc_rels,
            REL_NUMBERING,
            doc_part_name.as_str(),
            |bytes, _part| parse_numbering(bytes),
        )?;

        let raw_footnotes = resolve_optional_part(
            &package,
            doc_rels,
            REL_FOOTNOTES,
            doc_part_name.as_str(),
            |bytes, part| parse_notes(bytes, part),
        )?;

        let raw_endnotes = resolve_optional_part(
            &package,
            doc_rels,
            REL_ENDNOTES,
            doc_part_name.as_str(),
            |bytes, part| parse_notes(bytes, part),
        )?;

        // Settings are parsed but not yet used downstream.
        let _raw_settings = resolve_optional_part(
            &package,
            doc_rels,
            REL_SETTINGS,
            doc_part_name.as_str(),
            |bytes, _part| parse_settings(bytes),
        )?;

        // ── Build hyperlinks and images maps ──────────────────────────────
        let mut hyperlinks: HashMap<String, String> = HashMap::new();
        let mut images: HashMap<String, PartData> = HashMap::new();

        if let Some(rels) = doc_rels {
            for rel in rels.by_type(REL_HYPERLINK) {
                hyperlinks.insert(rel.id.clone(), rel.target.clone());
            }

            if self.options.embed_images {
                for rel in rels.by_type(REL_IMAGE) {
                    if let Ok(img_name) =
                        resolve_part_name(doc_part_name.as_str(), &rel.target)
                    {
                        if let Some(part) = package.part(&img_name) {
                            images.insert(rel.id.clone(), part.clone());
                        }
                    }
                }
            }
        }

        // ── Map everything to the abstract model ──────────────────────────
        let (document, warnings) = map_document(
            &raw_doc,
            &raw_styles,
            raw_numbering.as_ref(),
            raw_footnotes.as_ref(),
            raw_endnotes.as_ref(),
            images,
            hyperlinks,
            package.core_properties(),
            &self.options,
        );

        Ok(DocxImportResult { document, warnings })
    }
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
    let dir = base.rfind('/').map(|i| &base[..=i]).unwrap_or("/");
    PartName::new(format!("{dir}{target}")).map_err(OoxmlError::Opc)
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
    let rels = match doc_rels {
        Some(r) => r,
        None => return Ok(None),
    };
    let rel = match rels.by_type(rel_type).next() {
        Some(r) => r,
        None => return Ok(None),
    };
    let part_name = resolve_part_name(base_part, &rel.target)?;
    let part = match package.part(&part_name) {
        Some(p) => p,
        None => return Ok(None),
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
