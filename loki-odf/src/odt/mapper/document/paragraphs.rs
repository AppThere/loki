// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-level mapping: ODF paragraph → [`Block`].

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::style::catalog::StyleId;

use crate::odt::model::paragraph::OdfParagraph;

use super::context::OdfMappingContext;
use super::inlines::map_inline_children;

/// Convert an [`OdfParagraph`] to either [`Block::Heading`] (when
/// `is_heading` and `emit_heading_blocks` are both true) or
/// [`Block::StyledPara`].
pub(crate) fn map_paragraph(para: &OdfParagraph, ctx: &mut OdfMappingContext<'_>) -> Block {
    let inlines = map_inline_children(&para.children, ctx);

    if para.is_heading && ctx.options.emit_heading_blocks {
        let level = para.outline_level.unwrap_or(1).clamp(1, 6);
        // Store the ODF style name in NodeAttr so the layout engine can look up
        // heading style properties from the catalog. Without this, the flow engine
        // falls back to hardcoded "Heading1"/"Heading2" IDs which don't match ODF
        // names like "Heading_20_1" (LibreOffice-encoded space).
        let mut attr = NodeAttr::default();
        if let Some(ref sn) = para.style_name {
            attr.kv.push(("style".to_string(), sn.clone()));
        }
        Block::Heading(level, attr, inlines)
    } else {
        let style_id = para.style_name.as_deref().map(StyleId::new);
        Block::StyledPara(StyledParagraph {
            style_id,
            direct_para_props: None,
            direct_char_props: None,
            inlines,
            attr: NodeAttr::default(),
        })
    }
}
