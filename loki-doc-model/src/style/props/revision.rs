// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Editor-facing tracked-change (revision) marks (Spec 04 M5 Review tab, 4a.2).
//!
//! A [`RevisionMark`] tags a text run as a **live** tracked insertion or
//! deletion — the representation the editor's Review tab records, renders, and
//! accepts/rejects. It lives on [`CharProps::revision`][crate::style::props::char_props::CharProps::revision]
//! so it rides a text range as a CRDT mark (like highlight colour), keeping the
//! paragraph editable, unlike the opaque, round-trip-only
//! [`TrackedChange`][crate::content::annotation::TrackedChange].
//!
//! The CRDT mark value is a single string; [`encode`]/[`decode`] pack the fields
//! with `US` (unit-separator) delimiters so no serde/chrono is needed on the hot
//! bridge path and author names round-trip verbatim.

/// The field delimiter for the packed mark string — the ASCII Unit Separator,
/// which never appears in author names, ISO dates, or ids.
const SEP: char = '\u{1f}';

/// Whether a tracked run was inserted or deleted (the two edit kinds the editor
/// records; format-change/move stay in the opaque
/// [`TrackedChange`][crate::content::annotation::TrackedChange] model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RevisionKind {
    /// The run was inserted (rendered underlined in the author's colour;
    /// **kept** on accept, **removed** on reject). OOXML `w:ins` / ODF insertion.
    Insertion,
    /// The run was deleted (rendered struck-through; **removed** on accept,
    /// **kept** on reject). OOXML `w:del` / ODF deletion.
    Deletion,
}

impl RevisionKind {
    /// The one-char tag used in the packed mark string.
    #[must_use]
    fn tag(self) -> char {
        match self {
            RevisionKind::Insertion => 'i',
            RevisionKind::Deletion => 'd',
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "i" => Some(RevisionKind::Insertion),
            "d" => Some(RevisionKind::Deletion),
            _ => None,
        }
    }
}

/// A tracked-change mark on a run: what kind of change, and by whom / when.
///
/// `author`, `date` (RFC-3339 text), and `id` (the change group) are optional —
/// only `kind` is needed to accept/reject; the rest drive author colouring, the
/// change tooltip, and per-change accept/reject.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RevisionMark {
    /// Whether the run was inserted or deleted.
    pub kind: RevisionKind,
    /// The author who made the change (OOXML `w:author` / ODF `dc:creator`).
    pub author: Option<String>,
    /// When the change was made, as RFC-3339 text (OOXML `w:date` / ODF `dc:date`).
    pub date: Option<String>,
    /// The change-group id, so runs of one edit accept/reject together.
    pub id: Option<String>,
}

impl RevisionMark {
    /// A bare mark of the given `kind` with no author/date/id.
    #[must_use]
    pub fn new(kind: RevisionKind) -> Self {
        Self {
            kind,
            author: None,
            date: None,
            id: None,
        }
    }

    /// Sets the author (builder style).
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }
}

/// Packs a [`RevisionMark`] into its CRDT mark string:
/// `"{tag}␟{author}␟{date}␟{id}"` (empty field = absent).
#[must_use]
pub fn encode(mark: &RevisionMark) -> String {
    let field = |o: &Option<String>| o.clone().unwrap_or_default();
    format!(
        "{}{SEP}{}{SEP}{}{SEP}{}",
        mark.kind.tag(),
        field(&mark.author),
        field(&mark.date),
        field(&mark.id),
    )
}

/// Parses a packed mark string back into a [`RevisionMark`]. Returns `None` when
/// the tag is missing/unknown, so an unparseable mark is simply ignored.
#[must_use]
pub fn decode(s: &str) -> Option<RevisionMark> {
    let mut parts = s.split(SEP);
    let kind = RevisionKind::from_tag(parts.next()?)?;
    let non_empty = |p: Option<&str>| p.filter(|s| !s.is_empty()).map(str::to_string);
    Some(RevisionMark {
        kind,
        author: non_empty(parts.next()),
        date: non_empty(parts.next()),
        id: non_empty(parts.next()),
    })
}

#[cfg(test)]
#[path = "revision_tests.rs"]
mod tests;
