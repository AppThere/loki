// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Generated content blocks: captions, TOC, index, and notes.

use super::Block;
use crate::content::attr::NodeAttr;
use crate::content::inline::Inline;

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
