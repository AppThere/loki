// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document-level mapper: converts the ODF intermediate representation into
//! the format-neutral [`loki_doc_model::Document`].
//!
//! [`map_document`] is the entry point (called by `OdtImporter::run` after all
//! XML parts are parsed): it maps the stylesheet + list styles into a
//! [`StyleCatalog`], the body into [`Block`]s, the active master page into a
//! page layout, and the metadata into document metadata. The recursive-descent
//! helpers live in sibling modules — [`inlines`] (paragraphs, runs, fields),
//! [`frames`] (images / objects), [`blocks`] (lists, tables, sections),
//! [`page`] (page layout), and [`meta`] (document metadata).

use std::collections::HashMap;

use loki_doc_model::content::annotation::Comment;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::float::FloatWrap;
use loki_doc_model::document::Document;
use loki_doc_model::style::PageStyle;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_primitives::units::Points;

use crate::error::OdfWarning;
use crate::odt::import::OdtImportOptions;
use crate::odt::mapper::lists::map_list_styles;
use crate::odt::mapper::styles::map_stylesheet;
use crate::odt::model::document::{OdfBodyChild, OdfDocument, OdfMeta};
use crate::odt::model::revision::OdfChangedRegion;
use crate::odt::model::styles::{OdfCellProps, OdfStyle, OdfStylesheet};
use crate::xml_util::parse_length;

mod blocks;
mod frames;
mod inlines;
mod meta;
mod page;
mod sections;

use blocks::{map_list, map_section, map_table, map_toc};
use frames::map_graphic_wrap;
use inlines::map_paragraph;
use meta::map_meta;

// ── Context ────────────────────────────────────────────────────────────────────

/// State threaded through all mapping helpers during a single
/// [`map_document`] call: read-only references to the resolved catalog, images,
/// and options, plus mutable collectors for warnings, floating figures, and
/// comments.
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
    /// Column widths from `style:table-column-properties` (pre-built): name → pt.
    pub col_style_widths: &'a HashMap<String, Points>,
    /// Cell properties from `style:table-cell-properties` (pre-built): name → props.
    pub cell_style_props: &'a HashMap<String, OdfCellProps>,
    /// Frame text-wrap from `style:graphic-properties` (pre-built): name → wrap.
    pub frame_wraps: &'a HashMap<String, FloatWrap>,
    /// Non-fatal issues accumulated during mapping.
    pub warnings: Vec<OdfWarning>,
    /// Floating frames (images and text boxes that are not `as-char` anchored)
    /// collected while mapping inline content. The caller flushes this after
    /// each paragraph or block.
    pub pending_figures: Vec<Block>,
    /// Comment bodies collected from `office:annotation` start anchors.
    pub comments: Vec<Comment>,
    /// Tracked-change regions keyed by `text:id`, matched to body milestones.
    pub changed_regions: &'a HashMap<String, OdfChangedRegion>,
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

    // ── 3. Build style lookup for master-page resolution ─────────────────────
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

    // ── 3b. Collect tracked-change regions (keyed by change id) ──────────────
    let changed_regions: HashMap<String, OdfChangedRegion> = doc
        .body_children
        .iter()
        .filter_map(|c| match c {
            OdfBodyChild::TrackedChanges(regions) => Some(regions),
            _ => None,
        })
        .flatten()
        .map(|r| (r.change_id.clone(), r.clone()))
        .collect();

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
            changed_regions: &changed_regions,
        };
        let sections = sections::build_sections(
            &doc.body_children,
            stylesheet,
            &all_styles,
            initial_master,
            &mut ctx,
        );
        (sections, ctx.warnings, ctx.comments)
    };

    // ── 4b. Register ODF master-page names as first-class page styles ─────────
    // (ADR-0012 Decision 2) so the panel shows real names and they round-trip;
    // each style's geometry is its first referencing section's layout.
    for section in &sections {
        if let Some(id) = &section.page_style {
            catalog.page_styles.entry(id.clone()).or_insert_with(|| {
                let mut ps = PageStyle::new(id.clone(), section.layout.clone());
                // Carry the master page's `style:display-name` only when distinct
                // from its `style:name` (else leave None; fabricating `Some(id)`
                // would shadow a later rename).
                ps.display_name = stylesheet
                    .master_pages
                    .iter()
                    .find(|m| m.name == id.as_str())
                    .and_then(|m| m.display_name.clone())
                    .filter(|dn| dn != id.as_str());
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

pub(super) fn map_body_child(
    child: &OdfBodyChild,
    ctx: &mut OdfMappingContext<'_>,
) -> Option<Block> {
    match child {
        OdfBodyChild::Paragraph(para) | OdfBodyChild::Heading(para) => {
            Some(map_paragraph(para, ctx))
        }
        OdfBodyChild::List(list) => Some(map_list(list, ctx)),
        OdfBodyChild::Table(table) => Some(map_table(table, ctx)),
        OdfBodyChild::TableOfContent(toc) => Some(map_toc(toc, ctx)),
        OdfBodyChild::Section(section) => Some(map_section(section, ctx)),
        // The region table produces no block; its content rides the milestones.
        OdfBodyChild::TrackedChanges(_) => None,
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
