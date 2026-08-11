// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Body-child dispatch (split from `mod.rs` for the 300-line ceiling): one
//! `OdfBodyChild` → its mapped block(s), and the sequence walk that follows
//! each mapped child with its pending figures.

use loki_doc_model::content::block::Block;

use crate::error::OdfWarning;
use crate::odt::model::document::OdfBodyChild;

use super::OdfMappingContext;
use super::blocks::{map_list, map_section, map_table, map_toc};
use super::inlines::map_paragraph;

pub(crate) fn map_body_children(
    children: &[OdfBodyChild],
    ctx: &mut OdfMappingContext<'_>,
) -> Vec<Block> {
    let mut blocks = Vec::new();
    for child in children {
        let mapped = map_body_child(child, ctx);
        let had_any = !mapped.is_empty();
        blocks.extend(mapped);
        if had_any {
            let figures = std::mem::take(&mut ctx.pending_figures);
            blocks.extend(figures);
        }
    }
    blocks
}

pub(crate) fn map_body_child(child: &OdfBodyChild, ctx: &mut OdfMappingContext<'_>) -> Vec<Block> {
    match child {
        OdfBodyChild::Paragraph(para) | OdfBodyChild::Heading(para) => {
            vec![map_paragraph(para, ctx)]
        }
        // A list maps to a flat run of styled paragraphs (§10 path A).
        OdfBodyChild::List(list) => map_list(list, 0, ctx),
        OdfBodyChild::Table(table) => vec![map_table(table, ctx)],
        OdfBodyChild::TableOfContent(toc) => vec![map_toc(toc, ctx)],
        OdfBodyChild::Section(section) => vec![map_section(section, ctx)],
        // The region table produces no block; its content rides the milestones.
        OdfBodyChild::TrackedChanges(_) => Vec::new(),
        OdfBodyChild::Other { element } => {
            ctx.warnings.push(OdfWarning::UnrecognisedElement {
                element: element.clone(),
                context: "body index block (unimplemented)".to_string(),
            });
            Vec::new()
        }
    }
}
