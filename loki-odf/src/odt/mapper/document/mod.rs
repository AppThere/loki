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

mod body;
mod context;
mod fields;
mod frames;
mod inlines;
mod lists;
pub(crate) mod meta;
mod page_layout;
mod paragraphs;
mod tables;
mod toc_section;

#[cfg(test)]
mod tests;

pub(crate) use context::OdfMappingContext;
pub(crate) use page_layout::resolve_master_page_name;

use std::collections::HashMap;

use loki_doc_model::content::block::Block;
use loki_doc_model::document::Document;
use loki_doc_model::layout::section::Section;
use loki_primitives::units::Points;

use crate::error::OdfWarning;
use crate::odt::import::OdtImportOptions;
use crate::odt::mapper::lists::map_list_styles;
use crate::odt::mapper::styles::map_stylesheet;
use crate::odt::model::document::{OdfBodyChild, OdfDocument, OdfMeta};
use crate::odt::model::styles::{OdfCellProps, OdfStyle, OdfStylesheet};
use crate::xml_util::parse_length;

use body::map_body_child;
use meta::map_meta;
use page_layout::resolve_page_layout_by_name;

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
    let (sections, warnings) = {
        let mut ctx = OdfMappingContext {
            styles: &catalog,
            images,
            options,
            col_style_widths: &col_style_widths,
            cell_style_props: &cell_style_props,
            warnings: Vec::new(),
            pending_figures: Vec::new(),
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
                sections.push(Section::with_layout_and_blocks(
                    layout,
                    std::mem::take(&mut current_blocks),
                ));
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
        sections.push(Section::with_layout_and_blocks(layout, current_blocks));

        (sections, ctx.warnings)
    };

    // ── 5. Map metadata ───────────────────────────────────────────────────────
    let doc_meta = meta.map(map_meta).unwrap_or_default();

    // ── 6. Build document (caller sets source) ────────────────────────────────
    let document = Document {
        meta: doc_meta,
        styles: catalog,
        sections,
        settings: None,
        source: None,
    };

    (document, warnings)
}
