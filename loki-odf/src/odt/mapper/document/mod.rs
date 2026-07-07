// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document-level mapper: converts the ODF intermediate representation into
//! the format-neutral [`loki_doc_model::Document`].
//!
//! # Entry point
//!
//! [`map_document`] is the top-level conversion function, called by
//! [`crate::odt::import::OdtImporter::run`] after all XML parts have been
//! parsed. It coordinates:
//!
//! 1. Stylesheet → [`StyleCatalog`] via [`super::styles::map_stylesheet`]
//! 2. List styles → inserted into the same [`StyleCatalog`]
//! 3. Body content → [`Block`]s via recursive descent helpers
//! 4. Active master page → [`PageLayout`]
//! 5. Metadata → [`DocumentMeta`]
//!
//! The recursive-descent helpers are split across sibling modules — [`inlines`]
//! (paragraphs, runs, fields), [`frames`] (images / objects), [`blocks`] (lists,
//! tables, sections), [`page`] (page layout), and [`meta`] (document metadata).
//! Each submodule pulls the shared imports and the [`OdfMappingContext`] in via
//! `use super::*`.

use std::collections::HashMap;

use loki_doc_model::content::annotation::Comment;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::float::FloatWrap;
use loki_doc_model::document::Document;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::{PageStyle, StyleId};
use loki_primitives::units::Points;

use crate::error::OdfWarning;
use crate::odt::import::OdtImportOptions;
use crate::odt::mapper::lists::map_list_styles;
use crate::odt::mapper::styles::map_stylesheet;
use crate::odt::model::document::{OdfBodyChild, OdfDocument, OdfMeta};
use crate::odt::model::styles::{OdfCellProps, OdfStyle, OdfStylesheet};
use crate::xml_util::parse_length;

mod blocks;
mod frames;
mod inlines;
mod meta;
mod page;

use blocks::{map_list, map_section, map_table, map_toc};
use frames::map_graphic_wrap;
use inlines::map_paragraph;
use meta::map_meta;
use page::{resolve_master_page_name, resolve_page_layout_by_name};

// ── Context ────────────────────────────────────────────────────────────────────

/// State threaded through all mapping helpers during a single
/// [`map_document`] call.
///
/// Holds read-only references to the resolved catalog, image store, and import
/// options, plus mutable collections for warnings and for floating figures that
/// were encountered inside inline content and need to be emitted as block-level
/// siblings after their host paragraph.
pub(crate) struct OdfMappingContext<'a> {
    /// The fully-built style catalog (paragraph, character, list styles).
    pub styles: &'a StyleCatalog,
    /// Images extracted from the ODF package: ZIP-entry path →
    /// (media-type, raw bytes).
    pub images: &'a HashMap<String, (String, Vec<u8>)>,
    /// Embedded object sub-documents (e.g. formulas): object directory →
    /// raw `content.xml` bytes.
    pub objects: &'a HashMap<String, Vec<u8>>,
    /// Import options controlling heading emission, image embedding, etc.
    pub options: &'a OdtImportOptions,
    /// Column widths from `style:table-column-properties`: style name → points.
    /// Pre-built from the ODF stylesheet before the mapping pass.
    pub col_style_widths: &'a HashMap<String, Points>,
    /// Cell properties from `style:table-cell-properties`: style name → props.
    /// Pre-built from the ODF stylesheet before the mapping pass.
    pub cell_style_props: &'a HashMap<String, OdfCellProps>,
    /// Frame text-wrap from `style:graphic-properties`: graphic-style name →
    /// wrap config. Pre-built from the ODF stylesheet before the mapping pass.
    pub frame_wraps: &'a HashMap<String, FloatWrap>,
    /// Non-fatal issues accumulated during mapping.
    pub warnings: Vec<OdfWarning>,
    /// Floating frames (images and text boxes that are not `as-char` anchored)
    /// collected while mapping inline content. The caller flushes this after
    /// each paragraph or block.
    pub pending_figures: Vec<Block>,
    /// Comment bodies collected from `office:annotation` start anchors.
    pub comments: Vec<Comment>,
}

// ── Public entry point ─────────────────────────────────────────────────────────

/// Convert a fully-parsed ODF document into a format-neutral
/// [`loki_doc_model::Document`] plus a list of non-fatal [`OdfWarning`]s.
///
/// The [`crate::odt::import::OdtImporter`] calls this after reading all
/// package parts. The returned document's `source` field is left `None`;
/// the caller sets it with the correct [`OdfVersion`]-derived string.
///
/// [`OdfVersion`]: crate::version::OdfVersion
pub(crate) fn map_document(
    doc: &OdfDocument,
    stylesheet: &OdfStylesheet,
    meta: Option<&OdfMeta>,
    images: &HashMap<String, (String, Vec<u8>)>,
    objects: &HashMap<String, Vec<u8>>,
    options: &OdtImportOptions,
) -> (Document, Vec<OdfWarning>) {
    // ── 1. Map stylesheet + list styles ──────────────────────────────────────
    let mut catalog = map_stylesheet(stylesheet);
    map_list_styles(&stylesheet.list_styles, &mut catalog, doc.version);

    // ── 2. Pre-build column-width lookup from table-column styles ────────────────
    let col_style_widths: HashMap<String, Points> = stylesheet
        .named_styles
        .iter()
        .chain(stylesheet.auto_styles.iter())
        .filter_map(|s| {
            let width_str = s.col_width.as_deref()?;
            let pts = parse_length(width_str)?;
            Some((s.name.clone(), pts))
        })
        .collect();

    // ── 2b. Pre-build cell-style lookup from table-cell styles ───────────────
    let cell_style_props: HashMap<String, OdfCellProps> = stylesheet
        .named_styles
        .iter()
        .chain(stylesheet.auto_styles.iter())
        .filter_map(|s| Some((s.name.clone(), s.cell_props.clone()?)))
        .collect();

    // ── 2c. Pre-build frame-wrap lookup from graphic styles ──────────────────
    let frame_wraps: HashMap<String, FloatWrap> = stylesheet
        .named_styles
        .iter()
        .chain(stylesheet.auto_styles.iter())
        .filter_map(|s| Some((s.name.clone(), map_graphic_wrap(s.graphic_wrap.as_ref()?)?)))
        .collect();

    // ── 3. Build style lookup for master page resolution ─────────────────────
    let all_styles: HashMap<&str, &OdfStyle> = stylesheet
        .named_styles
        .iter()
        .chain(stylesheet.auto_styles.iter())
        .map(|s| (s.name.as_str(), s))
        .collect();

    // Identify the initial master page name ("Standard" > "Default" > first).
    let initial_master: Option<&str> = stylesheet
        .master_pages
        .iter()
        .find(|m| m.name == "Standard" || m.name == "Default")
        .or_else(|| stylesheet.master_pages.first())
        .map(|m| m.name.as_str());

    // ── 4. Map body, detecting master page transitions → multiple sections ────
    let (sections, warnings, comments) = {
        let mut ctx = OdfMappingContext {
            styles: &catalog,
            images,
            objects,
            options,
            col_style_widths: &col_style_widths,
            cell_style_props: &cell_style_props,
            frame_wraps: &frame_wraps,
            warnings: Vec::new(),
            pending_figures: Vec::new(),
            comments: Vec::new(),
        };

        let mut current_master: Option<String> = initial_master.map(str::to_string);
        let mut current_blocks: Vec<Block> = Vec::new();
        let mut sections: Vec<Section> = Vec::new();

        for child in &doc.body_children {
            // Only paragraphs/headings carry style:master-page-name.
            let new_master = match child {
                OdfBodyChild::Paragraph(para) | OdfBodyChild::Heading(para) => para
                    .style_name
                    .as_deref()
                    .and_then(|sn| resolve_master_page_name(sn, &all_styles)),
                _ => None,
            };

            // Emit a section break only when the master page actually changes.
            if let Some(ref nm) = new_master
                && Some(nm.as_str()) != current_master.as_deref()
            {
                let layout =
                    resolve_page_layout_by_name(stylesheet, current_master.as_deref(), &mut ctx);
                let mut section =
                    Section::with_layout_and_blocks(layout, std::mem::take(&mut current_blocks));
                // The finished section used `current_master` — store it as the
                // section's page style (ADR-0012 Decision 2's ODF-native mapping).
                section.page_style = current_master.as_deref().map(StyleId::new);
                sections.push(section);
                current_master = Some(nm.clone());
            }

            if let Some(block) = map_body_child(child, &mut ctx) {
                current_blocks.push(block);
                let figs = std::mem::take(&mut ctx.pending_figures);
                current_blocks.extend(figs);
            }
        }

        // Flush the final (or only) section.
        let layout = resolve_page_layout_by_name(stylesheet, current_master.as_deref(), &mut ctx);
        let mut section = Section::with_layout_and_blocks(layout, current_blocks);
        section.page_style = current_master.as_deref().map(StyleId::new);
        sections.push(section);

        (sections, ctx.warnings, ctx.comments)
    };

    // ── 4b. Register ODF master-page names as first-class page styles ─────────
    // (ADR-0012 Decision 2). The panel then shows the document's real page-style
    // names and they round-trip on export; the geometry is each style's first
    // referencing section's layout (the representative the panel reads too).
    for section in &sections {
        if let Some(id) = &section.page_style {
            catalog.page_styles.entry(id.clone()).or_insert_with(|| {
                let mut ps = PageStyle::new(id.clone(), section.layout.clone());
                ps.display_name = Some(id.as_str().to_string());
                ps
            });
        }
    }

    // ── 5. Map metadata ───────────────────────────────────────────────────────
    let doc_meta = meta.map(map_meta).unwrap_or_default();

    // ── 6. Build document (caller sets source) ────────────────────────────────
    let document = Document {
        meta: doc_meta,
        styles: catalog,
        sections,
        settings: None,
        comments,
        source: None,
    };

    (document, warnings)
}

// ── Body ───────────────────────────────────────────────────────────────────────

/// Convert a slice of [`OdfBodyChild`]s into [`Block`]s, flushing any
/// pending floating figures after each block.
pub(super) fn map_body_children(
    children: &[OdfBodyChild],
    ctx: &mut OdfMappingContext<'_>,
) -> Vec<Block> {
    let mut blocks = Vec::new();
    for child in children {
        if let Some(block) = map_body_child(child, ctx) {
            blocks.push(block);
            let figures = std::mem::take(&mut ctx.pending_figures);
            blocks.extend(figures);
        }
    }
    blocks
}

fn map_body_child(child: &OdfBodyChild, ctx: &mut OdfMappingContext<'_>) -> Option<Block> {
    match child {
        OdfBodyChild::Paragraph(para) | OdfBodyChild::Heading(para) => {
            Some(map_paragraph(para, ctx))
        }
        OdfBodyChild::List(list) => Some(map_list(list, ctx)),
        OdfBodyChild::Table(table) => Some(map_table(table, ctx)),
        OdfBodyChild::TableOfContent(toc) => Some(map_toc(toc, ctx)),
        OdfBodyChild::Section(section) => Some(map_section(section, ctx)),
        OdfBodyChild::Other { element } => {
            ctx.warnings.push(OdfWarning::UnrecognisedElement {
                element: element.clone(),
                context: "body index block (unimplemented)".to_string(),
            });
            None
        }
    }
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
