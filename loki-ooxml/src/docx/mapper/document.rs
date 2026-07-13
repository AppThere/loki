// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level document mapper: orchestrates all sub-mappers and produces a
//! [`loki_doc_model::Document`].

use std::collections::HashMap;

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::document::Document;
use loki_doc_model::layout::section::Section;
use loki_doc_model::meta::core::DocumentMeta;
use loki_doc_model::settings::DocumentSettings;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_opc::PartData;

use crate::docx::import::DocxImportOptions;
use crate::docx::model::document::{DocxBodyChild, DocxDocument};
use crate::docx::model::footnotes::{DocxNoteType, DocxNotes};
use crate::docx::model::numbering::DocxNumbering;
use crate::docx::model::paragraph::DocxParagraph;
use crate::docx::model::settings::DocxSettings;
use crate::docx::model::styles::DocxStyles;
use crate::error::OoxmlWarning;

use super::numbering::map_numbering;
use super::paragraph::map_paragraph;
use super::styles::map_styles;
use super::table::map_table;

#[path = "document_page.rs"]
mod page;
use page::{map_page_layout_with_hf, map_section_start};

// ── Context ───────────────────────────────────────────────────────────────────

/// Shared state threaded through all content-level mappers.
pub(crate) struct MappingContext<'a> {
    /// The resolved style catalog for this document.
    pub styles: &'a StyleCatalog,
    /// Footnote content keyed by `w:footnote @w:id`.
    pub footnotes: &'a HashMap<i32, Vec<Block>>,
    /// Endnote content keyed by `w:endnote @w:id`.
    pub endnotes: &'a HashMap<i32, Vec<Block>>,
    /// External hyperlink targets: relationship id → URL.
    pub hyperlinks: &'a HashMap<String, String>,
    /// Image parts: relationship id → raw bytes + media type.
    pub images: &'a HashMap<String, PartData>,
    /// Import options controlling heading promotion, image embedding, etc.
    pub options: &'a DocxImportOptions,
    /// Non-fatal warnings accumulated during mapping.
    pub warnings: Vec<OoxmlWarning>,
    /// Stack of currently open bookmark IDs and their names, to resolve names at `BookmarkEnd`.
    pub open_bookmarks: Vec<(String, String)>,
}

// ── Note pre-processing ───────────────────────────────────────────────────────

/// Maps a notes part to a `HashMap<id, Vec<Block>>` using the given context.
///
/// Only `Normal`-type notes are included; separators and continuation
/// separators are skipped. The context should use empty note maps to avoid
/// circular dependencies (notes referencing notes is not supported in v0.1.0).
fn map_notes_to_blocks(
    notes: Option<&DocxNotes>,
    ctx: &mut MappingContext<'_>,
) -> HashMap<i32, Vec<Block>> {
    let Some(notes) = notes else {
        return HashMap::new();
    };
    notes
        .notes
        .iter()
        .filter(|n| n.note_type == DocxNoteType::Normal)
        .map(|n| {
            let blocks: Vec<Block> = n
                .paragraphs
                .iter()
                .flat_map(|p| map_paragraph(p, ctx))
                .collect();
            (n.id, blocks)
        })
        .collect()
}

// ── Metadata ──────────────────────────────────────────────────────────────────

/// Populates [`DocumentMeta`] from OPC core properties.
fn map_meta(core_props: Option<&loki_opc::CoreProperties>) -> DocumentMeta {
    let Some(cp) = core_props else {
        return DocumentMeta::default();
    };
    let dublin_core = loki_doc_model::meta::dublin_core::DublinCoreMeta {
        identifier: cp.identifier.clone(),
        ..Default::default()
    };
    DocumentMeta {
        title: cp.title.clone(),
        creator: cp.creator.clone(),
        subject: cp.subject.clone(),
        keywords: cp.keywords.clone(),
        description: cp.description.clone(),
        last_modified_by: cp.last_modified_by.clone(),
        created: cp.created,
        modified: cp.modified,
        language: cp
            .language
            .as_ref()
            .map(|l| loki_doc_model::meta::language::LanguageTag::new(l.clone())),
        revision: cp.revision.as_ref().and_then(|r| r.parse().ok()),
        dublin_core,
        ..Default::default()
    }
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Orchestrates the full intermediate-model → [`Document`] translation.
///
/// Steps:
/// 1. Map styles (always present).
/// 2. Map numbering into the catalog (optional part).
/// 3. Pre-process footnote/endnote content maps.
/// 4. Walk body children, splitting on embedded `w:sectPr` elements.
/// 5. Close the final section using the body-level `w:sectPr`.
/// 6. Map core properties into document metadata.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn map_document(
    doc: &DocxDocument,
    raw_styles: &DocxStyles,
    raw_numbering: Option<&DocxNumbering>,
    raw_footnotes: Option<&DocxNotes>,
    raw_endnotes: Option<&DocxNotes>,
    images: &HashMap<String, PartData>,
    hyperlinks: &HashMap<String, String>,
    header_parts: &HashMap<String, Vec<DocxParagraph>>,
    footer_parts: &HashMap<String, Vec<DocxParagraph>>,
    raw_settings: Option<&DocxSettings>,
    core_props: Option<&loki_opc::CoreProperties>,
    options: &DocxImportOptions,
) -> (Document, Vec<OoxmlWarning>) {
    // ── 1. Styles ──────────────────────────────────────────────────────────
    let mut catalog = map_styles(raw_styles);
    let mut all_warnings: Vec<OoxmlWarning> = Vec::new();

    // ── 2. Numbering ───────────────────────────────────────────────────────
    if let Some(numbering) = raw_numbering {
        let w = map_numbering(numbering, &mut catalog);
        all_warnings.extend(w);
    }

    // ── 3. Notes ───────────────────────────────────────────────────────────
    // Map note paragraphs using a temporary context with empty note maps.
    // Notes that cross-reference other notes are not supported in v0.1.0.
    let empty_notes: HashMap<i32, Vec<Block>> = HashMap::new();
    let footnote_map = {
        let mut note_ctx = MappingContext {
            styles: &catalog,
            footnotes: &empty_notes,
            endnotes: &empty_notes,
            hyperlinks,
            images,
            options,
            warnings: Vec::new(),
            open_bookmarks: Vec::new(),
        };
        let result = map_notes_to_blocks(raw_footnotes, &mut note_ctx);
        all_warnings.extend(note_ctx.warnings);
        result
    };
    let endnote_map = {
        let mut note_ctx = MappingContext {
            styles: &catalog,
            footnotes: &empty_notes,
            endnotes: &empty_notes,
            hyperlinks,
            images,
            options,
            warnings: Vec::new(),
            open_bookmarks: Vec::new(),
        };
        let result = map_notes_to_blocks(raw_endnotes, &mut note_ctx);
        all_warnings.extend(note_ctx.warnings);
        result
    };

    let even_and_odd = raw_settings.is_some_and(|s| s.even_and_odd_headers);

    // ── 4+5. Section-split body walk ───────────────────────────────────────
    let mut ctx = MappingContext {
        styles: &catalog,
        footnotes: &footnote_map,
        endnotes: &endnote_map,
        hyperlinks,
        images,
        options,
        warnings: Vec::new(),
        open_bookmarks: Vec::new(),
    };

    let mut sections: Vec<Section> = Vec::new();
    let mut current_blocks: Vec<Block> = Vec::new();

    for child in &doc.body.children {
        match child {
            DocxBodyChild::Paragraph(p) => {
                let sect_pr = p.ppr.as_ref().and_then(|ppr| ppr.sect_pr.as_ref());
                let blocks = map_paragraph(p, &mut ctx);
                current_blocks.extend(blocks);
                if let Some(sp) = sect_pr {
                    let layout = map_page_layout_with_hf(
                        Some(sp),
                        header_parts,
                        footer_parts,
                        even_and_odd,
                        &mut ctx,
                    );
                    sections.push(Section {
                        layout,
                        blocks: super::drop_cap_merge::merge_drop_cap_frames(std::mem::take(
                            &mut current_blocks,
                        )),
                        start: map_section_start(sp.section_type.as_deref()),
                        page_style: None,
                        extensions: ExtensionBag::default(),
                    });
                }
            }
            DocxBodyChild::Table(t) => {
                let block = map_table(t, &mut ctx);
                current_blocks.push(block);
            }
        }
    }

    // Close the final section.
    let final_layout = map_page_layout_with_hf(
        doc.body.final_sect_pr.as_ref(),
        header_parts,
        footer_parts,
        even_and_odd,
        &mut ctx,
    );
    sections.push(Section {
        layout: final_layout,
        blocks: super::drop_cap_merge::merge_drop_cap_frames(current_blocks),
        start: map_section_start(
            doc.body
                .final_sect_pr
                .as_ref()
                .and_then(|sp| sp.section_type.as_deref()),
        ),
        page_style: None,
        extensions: ExtensionBag::default(),
    });

    all_warnings.extend(ctx.warnings);

    // ── 6. Metadata ────────────────────────────────────────────────────────
    let meta = map_meta(core_props);

    let tab = raw_settings.and_then(|s| s.default_tab_stop);
    let mirror = raw_settings.is_some_and(|s| s.mirror_margins);
    #[allow(clippy::cast_precision_loss)] // twips are small; f32 precision is sufficient
    let doc_settings = (tab.is_some() || mirror).then(|| DocumentSettings {
        default_tab_stop_pt: tab.map_or(36.0, |twips| twips as f32 / 20.0),
        mirror_margins: mirror,
        ..DocumentSettings::default()
    });

    let document = Document {
        meta,
        styles: catalog,
        sections,
        settings: doc_settings,
        // Comment bodies are merged from word/comments.xml by the importer.
        comments: Vec::new(),
        source: None,
    };

    (document, all_warnings)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
