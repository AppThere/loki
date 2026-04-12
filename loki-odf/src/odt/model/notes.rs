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
