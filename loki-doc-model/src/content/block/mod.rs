// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Block-level document elements.
//!
//! The core [`Block`] variants mirror `Text.Pandoc.Definition`'s `Block`
//! type exactly. Office-document-specific variants are added for styled
//! paragraphs, generated indices, and note collections.
//! See ADR-0001 for the design rationale.

mod block_enum;
mod generated;
mod list;
mod styled_para;

pub use block_enum::Block;
pub use generated::{Caption, IndexBlock, IndexKind, NotesBlockKind, TableOfContentsBlock};
pub use list::{ListAttributes, ListDelimiter, ListNumberStyle};
pub use styled_para::StyledParagraph;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::attr::NodeAttr;
    use crate::content::inline::Inline;
    use crate::style::catalog::StyleId;

    #[test]
    fn heading_level_one() {
        let block = Block::Heading(1, NodeAttr::default(), vec![Inline::Str("Title".into())]);
        assert!(matches!(block, Block::Heading(1, _, _)));
    }

    #[test]
    fn styled_para_with_style_ref() {
        let para = StyledParagraph {
            style_id: Some(StyleId("Normal".into())),
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Str("Hello".into())],
            attr: NodeAttr::default(),
        };
        let block = Block::StyledPara(para);
        if let Block::StyledPara(p) = &block {
            assert_eq!(p.style_id, Some(StyleId("Normal".into())));
            assert_eq!(p.inlines.len(), 1);
        } else {
            panic!("expected StyledPara");
        }
    }

    #[test]
    fn ordered_list_attributes_default() {
        let attrs = ListAttributes::default();
        assert_eq!(attrs.start_number, 1);
        assert_eq!(attrs.style, ListNumberStyle::Decimal);
    }
}
