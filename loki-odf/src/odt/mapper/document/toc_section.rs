// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table-of-contents and section block mapping.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, TableOfContentsBlock};

use crate::odt::model::document::{OdfSection, OdfTableOfContent};

use super::body::map_body_children;
use super::context::OdfMappingContext;
use super::paragraphs::map_paragraph;

pub(crate) fn map_toc(toc: &OdfTableOfContent, ctx: &mut OdfMappingContext<'_>) -> Block {
    let body: Vec<Block> = toc
        .body_paragraphs
        .iter()
        .flat_map(|p| {
            let block = map_paragraph(p, ctx);
            let figs = std::mem::take(&mut ctx.pending_figures);
            std::iter::once(block).chain(figs)
        })
        .collect();
    Block::TableOfContents(TableOfContentsBlock {
        title: None,
        body,
        attr: NodeAttr::default(),
    })
}

pub(crate) fn map_section(section: &OdfSection, ctx: &mut OdfMappingContext<'_>) -> Block {
    let blocks = map_body_children(&section.children, ctx);
    Block::Div(NodeAttr::default(), blocks)
}
