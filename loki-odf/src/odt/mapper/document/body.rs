// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Body-level mapping: ODF body children → [`Block`]s.

use loki_doc_model::content::block::Block;

use crate::error::OdfWarning;
use crate::odt::model::document::OdfBodyChild;

use super::context::OdfMappingContext;
use super::lists::map_list;
use super::paragraphs::map_paragraph;
use super::tables::map_table;
use super::toc_section::{map_section, map_toc};

/// Convert a slice of [`OdfBodyChild`]s into [`Block`]s, flushing any
/// pending floating figures after each block.
pub(crate) fn map_body_children(
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

pub(crate) fn map_body_child(
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
        OdfBodyChild::Other { element } => {
            ctx.warnings.push(OdfWarning::UnrecognisedElement {
                element: element.clone(),
                context: "body index block (unimplemented)".to_string(),
            });
            None
        }
    }
}
