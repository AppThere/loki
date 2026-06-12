// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Parse-and-map pipeline: reads all DOCX parts and produces the abstract model.

use std::collections::HashMap;

use loki_opc::{Package, PartData};

use crate::constants::{
    REL_ENDNOTES, REL_FOOTER, REL_FOOTNOTES, REL_HEADER, REL_HYPERLINK, REL_IMAGE, REL_NUMBERING,
    REL_OFFICE_DOCUMENT, REL_SETTINGS, REL_STYLES,
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

use super::helpers::{map_xml_err, rels_by_type, resolve_optional_part, resolve_part_name};
use super::options::DocxImportOptions;

/// Parses all DOCX parts from an open OPC [`Package`] and maps them to a
/// [`loki_doc_model::Document`].
///
/// Used by both [`super::importer::DocxImporter::run`] and the public
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

    let raw_styles = parse_styles_part(package, doc_rels, &doc_part_name)?;

    let raw_numbering = resolve_optional_part(
        package,
        doc_rels,
        REL_NUMBERING,
        doc_part_name.as_str(),
        |bytes, _part| parse_numbering(bytes),
    )?;

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

    // ── Build hyperlinks, images, headers, footers ────────────────────
    let (hyperlinks, images, header_parts, footer_parts) =
        collect_rel_resources(package, doc_rels, &doc_part_name, options)?;

    // ── Map everything to the abstract model ──────────────────────────
    let result = map_document(
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

    Ok(result)
}

/// Parses the optional styles part from the document relationships.
fn parse_styles_part(
    package: &Package,
    doc_rels: Option<&loki_opc::RelationshipSet>,
    doc_part_name: &loki_opc::PartName,
) -> OoxmlResult<crate::docx::model::styles::DocxStyles> {
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
    Ok(raw_styles.unwrap_or_default())
}

/// Collects hyperlinks, images, header paragraphs, and footer paragraphs from
/// the document part relationships.
fn collect_rel_resources(
    package: &Package,
    doc_rels: Option<&loki_opc::RelationshipSet>,
    doc_part_name: &loki_opc::PartName,
    options: &DocxImportOptions,
) -> OoxmlResult<(
    HashMap<String, String>,
    HashMap<String, PartData>,
    HashMap<String, Vec<DocxParagraph>>,
    HashMap<String, Vec<DocxParagraph>>,
)> {
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

    Ok((hyperlinks, images, header_parts, footer_parts))
}
