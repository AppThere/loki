// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Inline-level document elements.
//!
//! The core [`Inline`] variants mirror `Text.Pandoc.Definition`'s `Inline`
//! type exactly. Office-document-specific variants are added for styled runs,
//! fields, comments, and bookmarks. See ADR-0001 for the design rationale.

use crate::content::attr::NodeAttr;
use crate::content::block::Block;
use crate::content::field::types::Field;
use crate::content::annotation::comment::CommentRef;
use crate::style::catalog::StyleId;
use crate::style::props::char_props::CharProps;

/// The quotation mark style.
///
/// Corresponds to pandoc `QuoteType`. Used by [`Inline::Quoted`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QuoteType {
    /// Single quotation marks: 'text'.
    SingleQuote,
    /// Double quotation marks: "text".
    DoubleQuote,
}

/// Mathematical content display mode.
///
/// Corresponds to pandoc `MathType`. Used by [`Inline::Math`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MathType {
    /// Math embedded inline within a paragraph.
    InlineMath,
    /// Math displayed on its own line.
    DisplayMath,
}

/// The target of a hyperlink or cross-reference.
///
/// Corresponds to pandoc `Target = (Text, Text)` — `(url, title)`.
/// Used by [`Inline::Link`] and [`Inline::Image`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LinkTarget {
    /// The URL or target identifier. May be a full URL, a relative path,
    /// or a `#bookmark` anchor.
    pub url: String,
    /// An optional title attribute (shown as tooltip in HTML).
    pub title: Option<String>,
}

impl LinkTarget {
    /// Creates a [`LinkTarget`] with a URL and no title.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            title: None,
        }
    }
}

/// A citation reference. Corresponds to pandoc `Citation`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Citation {
    /// The citation key (e.g. a BibTeX key).
    pub id: String,
    /// Prefix inlines displayed before the citation (e.g. "see").
    pub prefix: Vec<Inline>,
    /// Suffix inlines displayed after the citation (e.g. "pp. 42").
    pub suffix: Vec<Inline>,
}

/// Whether a bookmark anchor is a start or end marker.
///
/// Used by [`Inline::Bookmark`].
/// ODF: `text:bookmark-start` / `text:bookmark-end` / `text:bookmark`.
/// OOXML: `w:bookmarkStart` / `w:bookmarkEnd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BookmarkKind {
    /// The beginning of a bookmarked range (or a point bookmark).
    Start,
    /// The end of a bookmarked range.
    End,
}

/// A styled inline run: inline content with an optional character style
/// reference and optional direct formatting overrides.
///
/// This is the primary run type produced by ODF and OOXML format parsers.
/// The pandoc-derived variants (`Strong`, `Emph`, etc.) are preserved for
/// pandoc AST compatibility. TR 29166 §6.2.1 and §7.2.2.
///
/// ODF: `text:span`. OOXML: `w:r` with `w:rPr`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StyledRun {
    /// Reference to a named character style in the style catalog.
    /// `None` = no character style; inherits from paragraph style.
    pub style_id: Option<StyleId>,
    /// Direct (inline) character formatting overrides.
    /// `None` = no direct formatting; all properties from the style.
    pub direct_props: Option<Box<CharProps>>,
    /// The inline content of this run.
    pub content: Vec<Inline>,
    /// Generic node attributes.
    pub attr: NodeAttr,
}

/// An inline-level document element.
///
/// Core variants mirror `Text.Pandoc.Definition`'s `Inline` type exactly.
/// Office-document-specific variants are added for features pandoc omits.
/// See ADR-0001 for the design rationale.
///
/// TR 29166 §7.2.2 (inline text structure).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Inline {
    // ── Pandoc-derived variants ─────────────────────────────────────────

    /// A plain text string. Corresponds to pandoc `Str`.
    Str(String),

    /// Emphasized (italic) text. Corresponds to pandoc `Emph`.
    Emph(Vec<Inline>),

    /// Underlined text. Corresponds to pandoc `Underline`.
    Underline(Vec<Inline>),

    /// Strong (bold) text. Corresponds to pandoc `Strong`.
    Strong(Vec<Inline>),

    /// Strikethrough text. Corresponds to pandoc `Strikeout`.
    Strikeout(Vec<Inline>),

    /// Superscript text. Corresponds to pandoc `Superscript`.
    Superscript(Vec<Inline>),

    /// Subscript text. Corresponds to pandoc `Subscript`.
    Subscript(Vec<Inline>),

    /// Small caps text. Corresponds to pandoc `SmallCaps`.
    SmallCaps(Vec<Inline>),

    /// Quoted text with a quote style. Corresponds to pandoc `Quoted`.
    Quoted(QuoteType, Vec<Inline>),

    /// A citation reference. Corresponds to pandoc `Cite`.
    Cite(Vec<Citation>, Vec<Inline>),

    /// Inline code. Corresponds to pandoc `Code`.
    Code(NodeAttr, String),

    /// Inter-word space. Corresponds to pandoc `Space`.
    Space,

    /// Soft line break (may be reflowed). Corresponds to pandoc `SoftBreak`.
    SoftBreak,

    /// Hard line break. Corresponds to pandoc `LineBreak`.
    LineBreak,

    /// Mathematical content. Corresponds to pandoc `Math`.
    Math(MathType, String),

    /// Raw inline in a specific format. Corresponds to pandoc `RawInline`.
    /// Use sparingly; prefer [`ExtensionBag`][crate::content::attr::ExtensionBag]
    /// for format-specific data.
    RawInline(String, String),

    /// A hyperlink or cross-reference. Corresponds to pandoc `Link`.
    /// ODF: `text:a`. OOXML: `w:hyperlink`.
    Link(NodeAttr, Vec<Inline>, LinkTarget),

    /// An inline image. Corresponds to pandoc `Image`.
    /// ODF: `draw:frame/draw:image` inline. OOXML: `w:drawing` inline.
    Image(NodeAttr, Vec<Inline>, LinkTarget),

    /// A footnote or endnote reference and its content.
    /// Corresponds to pandoc `Note`.
    /// ODF: `text:note`. OOXML: `w:footnote`/`w:endnote` reference.
    Note(Vec<Block>),

    /// A generic inline container with attributes. Corresponds to pandoc `Span`.
    Span(NodeAttr, Vec<Inline>),

    // ── Office-document extensions ──────────────────────────────────────

    /// A styled run: inline content with a character style reference and
    /// optional direct formatting overrides.
    ///
    /// This is the primary run type for office documents. See
    /// TR 29166 §6.2.1 and §7.2.2. ODF: `text:span`. OOXML: `w:r`.
    StyledRun(StyledRun),

    /// A document field — dynamic content evaluated at render time.
    /// ODF: `text:*` field elements. OOXML: `w:fldChar`/`w:instrText`.
    /// TR 29166 §5.2.19.
    Field(Field),

    /// A comment anchor (start/end of commented range).
    /// ODF: `office:annotation`. OOXML: `w:commentRangeStart`.
    Comment(CommentRef),

    /// A bookmark start or end marker.
    /// ODF: `text:bookmark-start`/`text:bookmark-end`.
    /// OOXML: `w:bookmarkStart`/`w:bookmarkEnd`.
    Bookmark(BookmarkKind, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_str() {
        let i = Inline::Str("hello".into());
        assert!(matches!(i, Inline::Str(ref s) if s == "hello"));
    }

    #[test]
    fn inline_styled_run_with_props() {
        let run = StyledRun {
            style_id: Some(StyleId("em".into())),
            direct_props: Some(Box::new(CharProps {
                italic: Some(true),
                ..Default::default()
            })),
            content: vec![Inline::Str("text".into())],
            attr: NodeAttr::default(),
        };
        let i = Inline::StyledRun(run);
        if let Inline::StyledRun(r) = &i {
            assert_eq!(r.direct_props.as_ref().map(|p| p.italic), Some(Some(true)));
        } else {
            panic!("expected StyledRun");
        }
    }

    #[test]
    fn inline_note_stores_blocks() {
        use crate::content::block::Block;
        let note = Inline::Note(vec![Block::HorizontalRule]);
        if let Inline::Note(blocks) = &note {
            assert_eq!(blocks.len(), 1);
        } else {
            panic!("expected Note");
        }
    }
}
