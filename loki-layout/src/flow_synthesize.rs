// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph synthesis for bare and heading blocks (split from `flow_tail.rs`
//! for the 300-line ceiling when the heading synthesizer learned to carry the
//! promotion-preserved direct page break).
//!
//! Both are `pub`, not `pub(super)`: ADR-0017's DOM reflow view needs the same
//! block → styled-paragraph mapping before it can resolve, and a second copy
//! would decide which style a heading level names for a second time.

use loki_doc_model::NodeAttr;
use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::Inline;

/// A bare inline run as the styled paragraph the resolver understands.
///
/// `pub`, not `pub(super)`: ADR-0017's DOM reflow view needs this same mapping
/// before it can resolve, and a second copy would decide which style a heading
/// level names for a second time.
pub fn synthesize_plain_para(inlines: &[Inline]) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

/// A heading as the styled paragraph the resolver understands. `pub` for the
/// same reason as [`synthesize_plain_para`].
pub fn synthesize_heading_para(level: u8, attr: &NodeAttr, inlines: &[Inline]) -> StyledParagraph {
    use loki_doc_model::content::heading::heading_style_id;
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
    // The style name carried in NodeAttr (set by the ODF mapper from
    // text:style-name so the catalog can resolve ODF heading properties like
    // font-size and bold), else the canonical OOXML/internal name.
    let style_id: StyleId = heading_style_id(level, attr);
    let direct_alignment =
        attr.kv
            .iter()
            .find(|(k, _)| k == "jc")
            .and_then(|(_, v)| match v.as_str() {
                "center" => Some(ParagraphAlignment::Center),
                "right" => Some(ParagraphAlignment::Right),
                "justify" => Some(ParagraphAlignment::Justify),
                _ => None,
            });
    // A direct page break carried through heading promotion (the OOXML mapper
    // sets it for "chapter starts a new page" headings).
    let page_break = attr
        .kv
        .iter()
        .any(|(k, v)| k == "page-break-before" && v == "true");
    let direct_para_props = (direct_alignment.is_some() || page_break).then(|| {
        Box::new(ParaProps {
            alignment: direct_alignment,
            page_break_before: page_break.then_some(true),
            ..Default::default()
        })
    });
    StyledParagraph {
        style_id: Some(style_id),
        direct_para_props,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}
