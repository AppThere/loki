// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Body → [`Section`] partitioning, splitting at ODF master-page transitions.
//!
//! A paragraph (or heading) whose style resolves a `style:master-page-name`
//! different from the running master page begins a new section on a new page —
//! the ODF equivalent of a Word section break (there is no explicit
//! `<w:sectPr>`; the transition is implicit). Extracted from `document/mod.rs`
//! to keep it under the file-size ceiling.

use std::collections::HashMap;

use loki_doc_model::content::block::Block;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::StyleId;

use super::page::{resolve_master_page_name, resolve_page_layout_by_name};
use super::{OdfMappingContext, map_body_child};
use crate::odt::model::document::OdfBodyChild;
use crate::odt::model::styles::{OdfStyle, OdfStylesheet};

/// Partition `body_children` into sections, opening a new one at each
/// master-page transition. `initial_master` is the document's opening master
/// page (Standard/Default/first); `all_styles` resolves a paragraph's
/// `style:master-page-name` through its parent chain.
pub(super) fn build_sections(
    body_children: &[OdfBodyChild],
    stylesheet: &OdfStylesheet,
    all_styles: &HashMap<&str, &OdfStyle>,
    initial_master: Option<&str>,
    ctx: &mut OdfMappingContext<'_>,
) -> Vec<Section> {
    let mut current_master: Option<String> = initial_master.map(str::to_string);
    let mut current_blocks: Vec<Block> = Vec::new();
    let mut sections: Vec<Section> = Vec::new();

    for child in body_children {
        // Only paragraphs/headings carry style:master-page-name.
        let new_master = match child {
            OdfBodyChild::Paragraph(para) | OdfBodyChild::Heading(para) => para
                .style_name
                .as_deref()
                .and_then(|sn| resolve_master_page_name(sn, all_styles)),
            _ => None,
        };

        // Emit a section break only when the master page actually changes.
        if let Some(ref nm) = new_master
            && Some(nm.as_str()) != current_master.as_deref()
        {
            // Flush the running section, unless nothing has accumulated yet: a
            // *leading* master declaration only sets the opening master (no empty
            // preceding section).
            if !current_blocks.is_empty() {
                flush_master_section(
                    &mut sections,
                    ctx,
                    stylesheet,
                    current_master.as_deref(),
                    std::mem::take(&mut current_blocks),
                );
            }
            current_master = Some(nm.clone());
        }

        if let Some(block) = map_body_child(child, ctx) {
            current_blocks.push(block);
            let figs = std::mem::take(&mut ctx.pending_figures);
            current_blocks.extend(figs);
        }
    }

    // Flush the final (or only) section.
    let master = current_master.as_deref();
    flush_master_section(&mut sections, ctx, stylesheet, master, current_blocks);
    sections
}

/// Resolve `master`'s page layout, wrap `blocks` in a [`Section`] carrying it
/// as the section's page style, and push it onto `sections`.
fn flush_master_section(
    sections: &mut Vec<Section>,
    ctx: &mut OdfMappingContext<'_>,
    stylesheet: &OdfStylesheet,
    master: Option<&str>,
    blocks: Vec<Block>,
) {
    let layout = resolve_page_layout_by_name(stylesheet, master, ctx);
    let mut section = Section::with_layout_and_blocks(layout, blocks);
    // ADR-0012 Decision 2: the finished section stores its master as page style.
    section.page_style = master.map(StyleId::new);
    sections.push(section);
}
