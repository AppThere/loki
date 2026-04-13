// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Block-level document elements.
//!
//! The core [`Block`] variants mirror `Text.Pandoc.Definition`'s `Block`
//! type exactly. Office-document-specific variants are added for styled
//! paragraphs, generated indices, and note collections.
//! See ADR-0001 for the design rationale.

use crate::content::attr::NodeAttr;
use crate::content::inline::Inline;
use crate::content::table::core::Table;
use crate::style::catalog::StyleId;
use crate::style::props::para_props::ParaProps;
use crate::style::props::char_props::CharProps;

/// The number style for an ordered list.
///
/// Corresponds to pandoc `ListNumberStyle`. Used by [`ListAttributes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ListNumberStyle {
    /// Use the default number style for the context.
    #[default]
    DefaultStyle,
    /// An example list (used for code examples in pandoc).
    Example,
    /// Arabic numerals: 1, 2, 3.
    Decimal,
    /// Lowercase Roman numerals: i, ii, iii.
    LowerRoman,
    /// Uppercase Roman numerals: I, II, III.
    UpperRoman,
    /// Lowercase Latin letters: a, b, c.
    LowerAlpha,
    /// Uppercase Latin letters: A, B, C.
    UpperAlpha,
}

/// The delimiter style around ordered list numbers.
///
/// Corresponds to pandoc `ListNumberDelim`. Used by [`ListAttributes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ListDelimiter {
    /// Use the default delimiter for the context.
    #[default]
    DefaultDelim,
    /// A period after the number: `1.`
    Period,
    /// A closing parenthesis: `1)`
    OneParen,
    /// Parentheses around the number: `(1)`
    TwoParens,
}

/// Attributes for an ordered list.
///
/// Corresponds to pandoc `ListAttributes = (Int, ListNumberStyle, ListNumberDelim)`.
/// Used by [`Block::OrderedList`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListAttributes {
    /// The starting number for the list.
    pub start_number: i32,
    /// The numbering style.
    pub style: ListNumberStyle,
    /// The delimiter style.
    pub delimiter: ListDelimiter,
}

impl Default for ListAttributes {
    fn default() -> Self {
        Self {
            start_number: 1,
            style: ListNumberStyle::Decimal,
            delimiter: ListDelimiter::Period,
        }
    }
}

/// A styled paragraph: paragraph content plus a style reference and optional
/// direct formatting overrides.
///
/// This is the primary paragraph type for office documents.
/// TR 29166 §7.2.2 (paragraph structure) and §7.2.3 (styles).
///
/// ODF: `text:p` with `text:style-name`. OOXML: `w:p` with `w:pStyle`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StyledParagraph {
    /// Reference to a named paragraph style in the style catalog.
    /// `None` = no named style; uses document defaults.
    pub style_id: Option<StyleId>,
    /// Direct paragraph formatting overrides.
    /// `None` = no direct formatting.
    pub direct_para_props: Option<Box<ParaProps>>,
    /// Direct character formatting overrides applied to the paragraph mark.
    pub direct_char_props: Option<Box<CharProps>>,
    /// The inline content of the paragraph.
    pub inlines: Vec<Inline>,
    /// Generic node attributes.
    pub attr: NodeAttr,
}

/// A figure caption.
///
/// Corresponds to pandoc `Caption = (Maybe ShortCaption, [Block])`.
/// Used by [`Block::Figure`].
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Caption {
    /// A short form of the caption for use in a list of figures.
    pub short: Option<Vec<Inline>>,
    /// The full caption as a sequence of blocks.
    pub full: Vec<Block>,
}

/// A generated table of contents block.
///
/// ODF: `text:table-of-content`. OOXML: `w:sdt` with a TOC field.
/// TR 29166 §6.2.7. Contains a cached snapshot of the rendered content.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableOfContentsBlock {
    /// A title for the TOC, if present.
    pub title: Option<Vec<Inline>>,
    /// The rendered TOC entries as a snapshot (may be stale).
    pub body: Vec<Block>,
    /// Generic node attributes.
    pub attr: NodeAttr,
}

/// A generated index block (alphabetical, subject, author, etc.).
///
/// ODF: `text:alphabetical-index` etc. OOXML: field-based.
/// TR 29166 §6.2.7 and §7.2.6.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexBlock {
    /// The kind of index.
    pub kind: IndexKind,
    /// The rendered index entries as a snapshot.
    pub body: Vec<Block>,
    /// Generic node attributes.
    pub attr: NodeAttr,
}

/// The type of generated index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum IndexKind {
    /// An alphabetical (subject) index.
    Alphabetical,
    /// An illustration (figure) index.
    Illustration,
    /// A table index.
    Table,
    /// An object index.
    Object,
    /// A bibliography.
    Bibliography,
    /// A user-defined index.
    UserDefined,
}

/// The kind of notes collection rendered by a [`Block::NotesBlock`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NotesBlockKind {
    /// A footnotes region (notes appear at the bottom of the page).
    Footnote,
    /// An endnotes region (notes appear at the end of the document).
    Endnote,
}

/// A block-level document element.
///
/// The core variants mirror `Text.Pandoc.Definition`'s `Block` type exactly.
/// Office-document-specific variants (`StyledPara`, `TableOfContents`,
/// `Index`, `NotesBlock`) are added for features that pandoc intentionally
/// omits as out of scope for a conversion-focused model.
///
/// TR 29166 §7.2.1 describes the logical structure that these blocks map
/// to in both ODF and OOXML. See ADR-0001.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Block {
    // ── Pandoc-derived variants ─────────────────────────────────────────

    /// A paragraph of plain inlines (no style reference).
    /// Corresponds to pandoc `Plain`.
    Plain(Vec<Inline>),

    /// A standard paragraph.
    /// ODF: `text:p`. OOXML: `w:p`.
    /// Corresponds to pandoc `Para`.
    Para(Vec<Inline>),

    /// Lines kept together, preserving explicit line breaks.
    /// ODF: `text:p` with `text:line-break`. OOXML: `w:p` with `w:br`.
    /// Corresponds to pandoc `LineBlock`.
    LineBlock(Vec<Vec<Inline>>),

    /// A preformatted code block. Monospace rendering implied.
    /// Corresponds to pandoc `CodeBlock`.
    CodeBlock(NodeAttr, String),

    /// Block content in a specific raw format, passed through opaquely.
    /// Corresponds to pandoc `RawBlock`. Format name, then content.
    RawBlock(String, String),

    /// A block-level quotation containing other blocks.
    /// Corresponds to pandoc `BlockQuote`.
    BlockQuote(Vec<Block>),

    /// An ordered list. Each item is a sequence of blocks.
    /// TR 29166 §6.2.5 and §7.2.5.
    /// Corresponds to pandoc `OrderedList`.
    OrderedList(ListAttributes, Vec<Vec<Block>>),

    /// An unordered (bullet) list. Each item is a sequence of blocks.
    /// Corresponds to pandoc `BulletList`.
    BulletList(Vec<Vec<Block>>),

    /// A definition list: `(term, [[definition block]])` pairs.
    /// Corresponds to pandoc `DefinitionList`.
    DefinitionList(Vec<(Vec<Inline>, Vec<Vec<Block>>)>),

    /// A section heading. Level 1–6.
    /// ODF: `text:h` with `text:outline-level`.
    /// OOXML: `w:p` with `w:pStyle` Heading1–Heading6.
    /// Corresponds to pandoc `Header`.
    Heading(u8, NodeAttr, Vec<Inline>),

    /// A horizontal rule or page divider.
    /// Corresponds to pandoc `HorizontalRule`.
    HorizontalRule,

    /// A table. See the `table/` module.
    /// TR 29166 §6.2.4 and §7.2.4.
    /// Corresponds to pandoc `Table`.
    /// Boxed to keep enum variant sizes balanced.
    Table(Box<Table>),

    /// A figure (block-level image with caption).
    /// ODF: `draw:frame/draw:image`. OOXML: `w:drawing`.
    /// Corresponds to pandoc `Figure` (added in pandoc-types 1.22).
    Figure(NodeAttr, Caption, Vec<Block>),

    /// A generic block container with attributes.
    /// Corresponds to pandoc `Div`.
    Div(NodeAttr, Vec<Block>),

    // ── Office-document extensions ──────────────────────────────────────

    /// A styled paragraph: paragraph content with a style reference and
    /// optional direct formatting overrides.
    ///
    /// The primary paragraph type for office document content.
    /// TR 29166 §7.2.2 and §7.2.3.
    StyledPara(StyledParagraph),

    /// A generated table of contents.
    /// ODF: `text:table-of-content`. OOXML: `w:sdt` with TOC field.
    /// TR 29166 §6.2.7.
    TableOfContents(TableOfContentsBlock),

    /// A generated index (alphabetical, subject, author, etc.).
    /// ODF: `text:alphabetical-index` etc. OOXML: field-based.
    /// TR 29166 §6.2.7 and §7.2.6.
    Index(IndexBlock),

    /// A footnotes or endnotes collection region.
    ///
    /// Represents the area where notes are rendered, not the note references.
    /// Note references appear as [`Inline::Note`].
    /// ODF: `text:notes-configuration`. OOXML: implicit in `w:footnote`.
    NotesBlock(NotesBlockKind),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_level_one() {
        let block = Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("Title".into())],
        );
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
