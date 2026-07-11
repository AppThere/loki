// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tracked-change (revision) model types.
//!
//! ODF represents a tracked change with a `<text:changed-region>` entry inside
//! the document-leading `<text:tracked-changes>` table (ODF 1.3 §5.5.3). Each
//! region carries an `<text:insertion>` or `<text:deletion>` wrapper with an
//! `<office:change-info>` (`dc:creator` / `dc:date`); a deletion additionally
//! holds the removed content. In the body, an insertion is bracketed by
//! `<text:change-start>` / `<text:change-end>` milestones and a deletion is
//! marked by a single `<text:change>` point, both keyed by `text:change-id`.

/// Whether a changed region records an insertion or a deletion. ODF 1.3 §5.5.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OdfChangeKind {
    /// `<text:insertion>` — the referenced body range was added.
    Insertion,
    /// `<text:deletion>` — the region holds the removed content.
    Deletion,
}

/// A single `<text:changed-region>` entry from the `<text:tracked-changes>`
/// table, keyed in the body by [`Self::change_id`].
#[derive(Debug, Clone)]
pub(crate) struct OdfChangedRegion {
    /// `text:id` — the change id referenced by the body milestones.
    pub change_id: String,
    /// Whether this region is an insertion or a deletion.
    pub kind: OdfChangeKind,
    /// `dc:creator` from the `office:change-info`, if present.
    pub creator: Option<String>,
    /// `dc:date` from the `office:change-info`, if present (ISO-8601 text).
    pub date: Option<String>,
    /// For a deletion, the plain text of the removed content (`text:p` bodies
    /// joined by `\n`); empty for an insertion.
    pub deleted_text: String,
}
