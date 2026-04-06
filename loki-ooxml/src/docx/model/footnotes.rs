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

//! Intermediate model for `word/footnotes.xml` and `word/endnotes.xml`.
//!
//! Mirrors ECMA-376 §17.11 (footnotes and endnotes).

use super::paragraph::DocxParagraph;

/// Top-level model for `w:footnotes` / `w:endnotes` (ECMA-376 §17.11.12 / §17.11.2).
#[derive(Debug, Clone, Default)]
pub struct DocxNotes {
    /// All note entries, including separators.
    pub notes: Vec<DocxNote>,
}

impl DocxNotes {
    /// Returns the content paragraphs for a given note id, if found.
    ///
    /// Skips separator and continuation-separator notes automatically.
    #[must_use]
    pub fn content_for(&self, id: i32) -> Option<&[DocxParagraph]> {
        self.notes
            .iter()
            .find(|n| n.id == id && n.note_type == DocxNoteType::Normal)
            .map(|n| n.paragraphs.as_slice())
    }
}

/// The type of a footnote/endnote entry (ECMA-376 §17.18.33).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocxNoteType {
    /// A normal content note (type absent or `"normal"`).
    #[default]
    Normal,
    /// The separator line (`type="separator"`).
    Separator,
    /// The continuation separator (`type="continuationSeparator"`).
    ContinuationSeparator,
}

/// A single footnote or endnote entry from `w:footnote` / `w:endnote`
/// (ECMA-376 §17.11.10 / §17.11.1).
#[derive(Debug, Clone)]
pub struct DocxNote {
    /// `@w:id` — unique identifier referenced from the document body.
    pub id: i32,
    /// `@w:type` — normal, separator, or continuation-separator.
    pub note_type: DocxNoteType,
    /// Content paragraphs.
    pub paragraphs: Vec<DocxParagraph>,
}
