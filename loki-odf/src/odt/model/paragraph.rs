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

//! ODF paragraph and inline content model types.
//!
//! Covers `text:p` (ODF 1.3 §5.1), `text:h` (§5.1), `text:span` (§6.1),
//! `text:a` (§6.4), and the inline child nodes that may appear within them.
//! Text nodes (plain character data) are represented by the
//! `OdfParagraphChild::Text` variant.

use super::fields::OdfField;
use super::frames::OdfFrame;
use super::notes::OdfNote;

/// An ODF text paragraph or heading.
///
/// Represents both `text:p` (paragraph, ODF 1.3 §5.1) and `text:h`
/// (heading, ODF 1.3 §5.1). The `is_heading` flag and `outline_level`
/// distinguish them.
#[derive(Debug, Clone)]
pub(crate) struct OdfParagraph {
    /// `text:style-name` — the named or automatic style applied to this
    /// paragraph.
    pub style_name: Option<String>,
    /// `text:outline-level` — heading depth (1–10). Only meaningful when
    /// `is_heading` is `true`. ODF 1.3 §19.857.
    pub outline_level: Option<u8>,
    /// `true` when this element is `text:h`; `false` for `text:p`.
    pub is_heading: bool,
    /// Inline children of this paragraph, in document order.
    pub children: Vec<OdfParagraphChild>,
    /// List context injected by the reader when this paragraph is inside a
    /// `text:list`. `None` for paragraphs outside any list.
    pub list_context: Option<OdfListContext>,
}

/// Context set by the list reader when a paragraph lives inside a
/// `text:list-item`. ODF 1.3 §5.3.
#[derive(Debug, Clone)]
pub(crate) struct OdfListContext {
    /// `text:style-name` from the nearest enclosing `text:list` element.
    pub style_name: Option<String>,
    /// 0-indexed nesting depth (0 = outermost list).
    pub level: u8,
    /// `xml:id` of the enclosing `text:list-item` (ODF 1.2+).
    pub item_id: Option<String>,
}

/// An inline child node within a paragraph or span.
///
/// ODF paragraphs contain a mix of text, styled spans, hyperlinks, fields,
/// and other inline elements. This enum captures every child kind that the
/// importer handles. ODF 1.3 §6.
#[derive(Debug, Clone)]
pub(crate) enum OdfParagraphChild {
    /// Plain text content (a text node in the XML).
    Text(String),

    /// A styled text span (`text:span`). ODF 1.3 §6.1.
    Span(OdfSpan),

    /// A hyperlink (`text:a`). ODF 1.3 §6.4.
    Hyperlink(OdfHyperlink),

    /// A footnote or endnote (`text:note`). ODF 1.3 §6.3.
    Note(OdfNote),

    /// A start bookmark marker (`text:bookmark`). ODF 1.3 §6.6.
    ///
    /// `id` is `xml:id` (ODF 1.2+); `name` is `text:name`.
    Bookmark {
        /// `xml:id` of the bookmark start, if present (ODF 1.2+).
        id: Option<String>,
        /// `text:name` — the logical name of the bookmark.
        name: String,
    },

    /// An end bookmark marker (`text:bookmark-end`). ODF 1.3 §6.6.
    BookmarkEnd {
        /// `text:name` — matches the corresponding `Bookmark`.
        name: String,
    },

    /// A computed text field. ODF 1.3 §12.
    Field(OdfField),

    /// An anchored drawing frame (`draw:frame`). ODF 1.3 §10.4.
    Frame(OdfFrame),

    /// A soft page break (`text:soft-page-break`). ODF 1.3 §5.6.
    SoftReturn,

    /// A tab character (`text:tab`). ODF 1.3 §6.7.
    Tab,

    /// One or more space characters (`text:s`). ODF 1.3 §6.8.
    ///
    /// `count` is the `text:c` attribute (default 1).
    Space {
        /// Number of spaces represented by this element.
        count: u32,
    },

    /// A hard line break (`text:line-break`). ODF 1.3 §6.5.
    LineBreak,

    /// Any inline element not specifically modelled above.
    Other,
}

/// A styled text span (`text:span`). ODF 1.3 §6.1.
#[derive(Debug, Clone)]
pub(crate) struct OdfSpan {
    /// `text:style-name` — the automatic character style applied to this span.
    pub style_name: Option<String>,
    /// Inline children of this span, in document order.
    pub children: Vec<OdfParagraphChild>,
}

/// A hyperlink anchor (`text:a`). ODF 1.3 §6.4.
#[derive(Debug, Clone)]
pub(crate) struct OdfHyperlink {
    /// `xlink:href` — the link target URI.
    pub href: Option<String>,
    /// `text:style-name` — character style applied to the link text.
    pub style_name: Option<String>,
    /// Inline children of this hyperlink anchor.
    pub children: Vec<OdfParagraphChild>,
}
