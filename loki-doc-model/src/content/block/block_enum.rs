// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The [`Block`] enum — block-level document elements.

use super::generated::{Caption, IndexBlock, NotesBlockKind, TableOfContentsBlock};
use super::list::ListAttributes;
use super::styled_para::StyledParagraph;
use crate::content::attr::NodeAttr;
use crate::content::inline::Inline;
use crate::content::table::core::Table;

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
