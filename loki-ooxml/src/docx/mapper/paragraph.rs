// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph mapper: `w:p` → `Vec<Block>`.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::para_props::ParagraphAlignment;

use crate::docx::model::paragraph::{DocxParaChild, DocxParagraph, DocxRunChild};

use super::document::MappingContext;
use super::inline::map_inlines;
use super::props::{map_ppr, map_rpr};

/// Maps a `w:p` paragraph to zero or more [`Block`]s.
///
/// Normally produces a single [`Block::StyledPara`]. When
/// [`DocxImportOptions::emit_heading_blocks`] is enabled and the paragraph
/// has an outline level, a [`Block::Heading`] is emitted first so that
/// consumers that prefer structural heading blocks can use them directly.
pub(crate) fn map_paragraph(p: &DocxParagraph, ctx: &mut MappingContext<'_>) -> Vec<Block> {
    let mut para_props = p.ppr.as_ref().map(map_ppr);

    // Detect `<w:br w:type="page"/>` inside any run child and promote it to
    // a paragraph-level page_break_after flag so the layout engine can honour it.
    let has_page_break = p.children.iter().any(|child| match child {
        DocxParaChild::Run(run) => run
            .children
            .iter()
            .any(|rc| matches!(rc, DocxRunChild::Break { break_type: Some(t) } if t == "page")),
        _ => false,
    });
    if has_page_break {
        para_props
            .get_or_insert_with(Default::default)
            .page_break_after = Some(true);
    }

    // Detect `<w:br w:type="column"/>` and promote to column_break_after.
    let has_column_break = p.children.iter().any(|child| match child {
        DocxParaChild::Run(run) => run
            .children
            .iter()
            .any(|rc| matches!(rc, DocxRunChild::Break { break_type: Some(t) } if t == "column")),
        _ => false,
    });
    if has_column_break {
        para_props
            .get_or_insert_with(Default::default)
            .column_break_after = Some(true);
    }

    let style_id = p
        .ppr
        .as_ref()
        .and_then(|ppr| ppr.style_id.as_ref())
        .map(|s| StyleId::new(s.clone()));

    let inlines = map_inlines(&p.children, ctx);

    // Word renders an equation that is the whole paragraph as *display* math:
    // centered on its own line. This is true both for an explicit `m:oMathPara`
    // (imported as `DisplayMath`) and for a bare paragraph-level `m:oMath` (no
    // surrounding runs — imported as `InlineMath` since context is unknown at
    // read time). So: when a paragraph's content is solely an equation (any math
    // kind, ignoring whitespace) and no explicit alignment was set, center it.
    let math_only_paragraph = {
        let mut maths = 0usize;
        let mut others = 0usize;
        for i in &inlines {
            match i {
                Inline::Math(_, _) => maths += 1,
                Inline::Str(s) if s.trim().is_empty() => {}
                Inline::Space | Inline::SoftBreak | Inline::LineBreak => {}
                _ => others += 1,
            }
        }
        maths >= 1 && others == 0
    };
    if math_only_paragraph && para_props.as_ref().is_none_or(|pp| pp.alignment.is_none()) {
        para_props.get_or_insert_with(Default::default).alignment =
            Some(ParagraphAlignment::Center);
    }

    // Determine outline level: direct props win; fall back to resolved style.
    let outline_level = para_props
        .as_ref()
        .and_then(|pp| pp.outline_level)
        .or_else(|| {
            style_id
                .as_ref()
                .and_then(|id| ctx.styles.resolve_para(id))
                .and_then(|pp| pp.outline_level)
        });

    if ctx.options.emit_heading_blocks
        && let Some(level) = outline_level
    {
        // Promote to a structural heading block.
        // Preserve any direct paragraph alignment in NodeAttr.kv so the
        // layout engine can restore it when synthesising the StyledParagraph.
        let mut attr = NodeAttr::default();
        if let Some(ref pp) = para_props {
            use loki_doc_model::style::props::para_props::ParagraphAlignment;
            if let Some(align) = pp.alignment {
                let val = match align {
                    ParagraphAlignment::Center => "center",
                    ParagraphAlignment::Right | ParagraphAlignment::Distribute => "right",
                    ParagraphAlignment::Justify => "justify",
                    _ => "left",
                };
                attr.kv.push(("jc".into(), val.into()));
            }
        }
        return vec![Block::Heading(level, attr, inlines)];
    }

    let direct_char_props = p
        .ppr
        .as_ref()
        .and_then(|ppr| ppr.ppr_rpr.as_ref())
        .map(|rpr| Box::new(map_rpr(rpr)));

    vec![Block::StyledPara(StyledParagraph {
        style_id,
        direct_para_props: para_props.map(Box::new),
        direct_char_props,
        inlines,
        attr: NodeAttr::default(),
    })]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "paragraph_tests.rs"]
mod tests;
