// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level document mapper: orchestrates all sub-mappers and produces a
//! [`loki_doc_model::Document`].

mod context;
mod meta;
mod notes;
mod page_layout;

pub(crate) use context::MappingContext;

use std::collections::HashMap;

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::document::Document;
use loki_doc_model::layout::section::Section;
use loki_doc_model::settings::DocumentSettings;
use loki_opc::PartData;

use crate::docx::import::DocxImportOptions;
use crate::docx::model::document::{DocxBodyChild, DocxDocument};
use crate::docx::model::footnotes::DocxNotes;
use crate::docx::model::numbering::DocxNumbering;
use crate::docx::model::paragraph::DocxParagraph;
use crate::docx::model::settings::DocxSettings;
use crate::docx::model::styles::DocxStyles;
use crate::error::OoxmlWarning;

use super::numbering::map_numbering;
use super::paragraph::map_paragraph;
use super::styles::map_styles;
use super::table::map_table;

use meta::map_meta;
use notes::map_notes_to_blocks;
use page_layout::map_page_layout_with_hf;

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
                        blocks: std::mem::take(&mut current_blocks),
                        extensions: ExtensionBag::default(),
                    });
                }
            }
            DocxBodyChild::Table(t) => {
                let block = map_table(t, &mut ctx);
                current_blocks.push(block);
            }
            DocxBodyChild::Sdt => {}
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
        blocks: current_blocks,
        extensions: ExtensionBag::default(),
    });

    all_warnings.extend(ctx.warnings);

    // ── 6. Metadata ────────────────────────────────────────────────────────
    let meta = map_meta(core_props);

    let doc_settings = raw_settings.and_then(|s| {
        #[allow(clippy::cast_precision_loss)] // twips are small values; f32 precision is sufficient
        s.default_tab_stop.map(|twips| DocumentSettings {
            default_tab_stop_pt: twips as f32 / 20.0,
        })
    });

    let document = Document {
        meta,
        styles: catalog,
        sections,
        settings: doc_settings,
        source: None,
    };

    (document, all_warnings)
}

#[cfg(test)]
mod tests;
