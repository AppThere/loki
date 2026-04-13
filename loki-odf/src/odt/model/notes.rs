// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! ODF footnote and endnote model types.
//!
//! The `text:note` element (ODF 1.3 §6.3) is used for both footnotes and
//! endnotes; the `text:note-class` attribute distinguishes them. A note
//! consists of a citation mark and a body containing one or more paragraphs.

use super::paragraph::OdfParagraph;

/// An ODF note (footnote or endnote). ODF 1.3 §6.3 `text:note`.
#[derive(Debug, Clone)]
pub(crate) struct OdfNote {
    /// `text:id` — unique note identifier within the document.
    pub id: Option<String>,
    /// Whether this is a footnote or endnote. ODF 1.3 §19.841
    /// `text:note-class`.
    pub note_class: OdfNoteClass,
    /// Text content of `text:note-citation` — the in-text call-out mark.
    pub citation: Option<String>,
    /// Paragraphs inside `text:note-body`.
    pub body: Vec<OdfParagraph>,
}

/// Whether a [`OdfNote`] is a footnote or an endnote.
///
/// ODF 1.3 §19.841 `text:note-class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OdfNoteClass {
    /// `text:note-class="footnote"` — appears at the bottom of the page.
    Footnote,
    /// `text:note-class="endnote"` — appears at the end of the document.
    Endnote,
}
