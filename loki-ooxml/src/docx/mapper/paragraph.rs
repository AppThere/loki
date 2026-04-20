// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Paragraph mapper: `w:p` → `Vec<Block>`.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::style::catalog::StyleId;

use crate::docx::model::paragraph::{DocxParagraph, DocxParaChild, DocxRunChild};

use super::document::MappingContext;
use super::inline::map_inlines;
use super::props::map_ppr;

/// Maps a `w:p` paragraph to zero or more [`Block`]s.
///
/// Normally produces a single [`Block::StyledPara`]. When
/// [`DocxImportOptions::emit_heading_blocks`] is enabled and the paragraph
/// has an outline level, a [`Block::Heading`] is emitted first so that
/// consumers that prefer structural heading blocks can use them directly.
pub(crate) fn map_paragraph(p: &DocxParagraph, ctx: &mut MappingContext<'_>) -> Vec<Block> {
    let mut para_props = p.ppr.as_ref().map(|ppr| map_ppr(ppr));

    // Detect `<w:br w:type="page"/>` inside any run child and promote it to
    // a paragraph-level page_break_after flag so the layout engine can honour it.
    let has_page_break = p.children.iter().any(|child| match child {
        DocxParaChild::Run(run) => run.children.iter().any(|rc| {
            matches!(rc, DocxRunChild::Break { break_type: Some(t) } if t == "page")
        }),
        _ => false,
    });
    if has_page_break {
        para_props
            .get_or_insert_with(Default::default)
            .page_break_after = Some(true);
    }

    let style_id = p.ppr.as_ref()
        .and_then(|ppr| ppr.style_id.as_ref())
        .map(|s| StyleId::new(s.clone()));

    let inlines = map_inlines(&p.children, ctx);

    // Determine outline level: direct props win; fall back to resolved style.
    let outline_level = para_props.as_ref().and_then(|pp| pp.outline_level).or_else(|| {
        style_id.as_ref()
            .and_then(|id| ctx.styles.resolve_para(id))
            .and_then(|pp| pp.outline_level)
    });

    if ctx.options.emit_heading_blocks {
        if let Some(level) = outline_level {
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
                        ParagraphAlignment::Left => "left",
                        _ => "left",
                    };
                    attr.kv.push(("jc".into(), val.into()));
                }
            }
            return vec![Block::Heading(level, attr, inlines)];
        }
    }

    vec![Block::StyledPara(StyledParagraph {
        style_id,
        direct_para_props: para_props.map(Box::new),
        direct_char_props: None,
        inlines,
        attr: NodeAttr::default(),
    })]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use loki_doc_model::content::inline::Inline;
    use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
    use loki_doc_model::style::para_style::ParagraphStyle;
    use loki_doc_model::style::props::para_props::ParaProps;
    use loki_doc_model::style::props::char_props::CharProps;
    use loki_doc_model::content::attr::ExtensionBag;
    use loki_opc::PartData;
    use crate::docx::import::DocxImportOptions;
    use crate::docx::model::paragraph::{DocxParaChild, DocxPPr, DocxRun, DocxRunChild};

    fn make_ctx<'a>(
        styles: &'a StyleCatalog,
        footnotes: &'a HashMap<i32, Vec<Block>>,
        endnotes: &'a HashMap<i32, Vec<Block>>,
        hyperlinks: &'a HashMap<String, String>,
        images: &'a HashMap<String, PartData>,
        options: &'a DocxImportOptions,
    ) -> MappingContext<'a> {
        MappingContext { styles, footnotes, endnotes, hyperlinks, images, options, warnings: Vec::new() }
    }

    fn text_child(s: &str) -> DocxParaChild {
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::Text { text: s.to_string(), preserve: false }],
        })
    }

    fn default_opts() -> DocxImportOptions {
        DocxImportOptions::default()
    }

    fn empty_maps() -> (HashMap<i32, Vec<Block>>, HashMap<i32, Vec<Block>>,
                         HashMap<String, String>, HashMap<String, PartData>) {
        (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new())
    }

    #[test]
    fn plain_paragraph_produces_styled_para() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = empty_maps();
        let opts = default_opts();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let p = DocxParagraph {
            ppr: None,
            children: vec![text_child("hello world")],
        };
        let blocks = map_paragraph(&p, &mut ctx);
        assert_eq!(blocks.len(), 1);
        if let Block::StyledPara(sp) = &blocks[0] {
            assert_eq!(sp.style_id, None);
            assert_eq!(sp.inlines, vec![Inline::Str("hello world".into())]);
        } else {
            panic!("expected StyledPara");
        }
    }

    #[test]
    fn paragraph_with_style_id() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = empty_maps();
        let opts = default_opts();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let p = DocxParagraph {
            ppr: Some(DocxPPr { style_id: Some("BodyText".into()), ..Default::default() }),
            children: vec![text_child("text")],
        };
        let blocks = map_paragraph(&p, &mut ctx);
        if let Block::StyledPara(sp) = &blocks[0] {
            assert_eq!(sp.style_id, Some(StyleId::new("BodyText")));
        } else {
            panic!("expected StyledPara");
        }
    }

    #[test]
    fn heading_via_direct_outline_level_emits_heading_block() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = empty_maps();
        let opts = DocxImportOptions { emit_heading_blocks: true, ..Default::default() };
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let p = DocxParagraph {
            ppr: Some(DocxPPr { outline_lvl: Some(0), ..Default::default() }), // 0 = Heading1 in OOXML (0-indexed)
            children: vec![text_child("Title")],
        };
        let blocks = map_paragraph(&p, &mut ctx);
        // Should produce [Heading(1, ...)]
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], Block::Heading(1, _, _)));
    }

    #[test]
    fn heading_via_style_outline_level() {
        let mut styles = StyleCatalog::default();
        let heading_style = ParagraphStyle {
            id: StyleId::new("Heading1"),
            display_name: Some("Heading 1".into()),
            parent: None,
            linked_char_style: None,
            para_props: ParaProps { outline_level: Some(1), ..Default::default() },
            char_props: CharProps::default(),
            is_default: false,
            is_custom: false,
            extensions: ExtensionBag::default(),
        };
        styles.paragraph_styles.insert(StyleId::new("Heading1"), heading_style);

        let (fn_m, en_m, hl_m, img_m) = empty_maps();
        let opts = DocxImportOptions { emit_heading_blocks: true, ..Default::default() };
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let p = DocxParagraph {
            ppr: Some(DocxPPr { style_id: Some("Heading1".into()), ..Default::default() }),
            children: vec![text_child("Chapter 1")],
        };
        let blocks = map_paragraph(&p, &mut ctx);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], Block::Heading(1, _, _)));
    }

    #[test]
    fn heading_suppressed_when_option_disabled() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = empty_maps();
        let opts = DocxImportOptions { emit_heading_blocks: false, ..Default::default() };
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let p = DocxParagraph {
            ppr: Some(DocxPPr { outline_lvl: Some(0), ..Default::default() }),
            children: vec![text_child("No heading block")],
        };
        let blocks = map_paragraph(&p, &mut ctx);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], Block::StyledPara(_)));
    }

    #[test]
    fn empty_paragraph_produces_empty_inlines() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = empty_maps();
        let opts = default_opts();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let p = DocxParagraph { ppr: None, children: vec![] };
        let blocks = map_paragraph(&p, &mut ctx);
        assert_eq!(blocks.len(), 1);
        if let Block::StyledPara(sp) = &blocks[0] {
            assert!(sp.inlines.is_empty());
        } else {
            panic!("expected StyledPara");
        }
    }
}
