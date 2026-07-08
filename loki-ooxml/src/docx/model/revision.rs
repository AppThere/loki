// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Intermediate model for DOCX tracked changes (`w:ins` / `w:del`).

use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

use crate::docx::model::paragraph::DocxRun;

/// A tracked change (`w:ins` / `w:del`): the change attributes plus the runs it
/// wraps. ECMA-376 §17.13.5.14/.16.
#[derive(Debug, Clone)]
pub struct DocxTrackedChange {
    /// `w:author` / `w:date` / `w:id` on the `w:ins` / `w:del` element.
    pub info: DocxRevisionInfo,
    /// The runs inside the tracked-change wrapper.
    pub runs: Vec<DocxRun>,
}

/// The `w:author` / `w:date` / `w:id` attributes of a tracked change.
#[derive(Debug, Clone, Default)]
pub struct DocxRevisionInfo {
    /// `w:author` — who made the change.
    pub author: Option<String>,
    /// `w:date` — when, as an ISO-8601 / RFC-3339 timestamp.
    pub date: Option<String>,
    /// `w:id` — the revision id.
    pub id: Option<String>,
}

/// A tracked-change mark on a paragraph mark's run properties
/// (`w:pPr/w:rPr/w:ins|del`) — the deleted/inserted ¶ itself (Review tab 4a.2).
#[derive(Debug, Clone)]
pub struct DocxMarkRevision {
    /// `true` for `w:del` (the ¶ is deleted), `false` for `w:ins`.
    pub is_deletion: bool,
    /// The `w:id` / `w:author` / `w:date` attributes.
    pub info: DocxRevisionInfo,
}

impl DocxMarkRevision {
    /// The format-neutral [`RevisionMark`] for this paragraph-mark change.
    #[must_use]
    pub fn to_mark(&self) -> RevisionMark {
        let kind = if self.is_deletion {
            RevisionKind::Deletion
        } else {
            RevisionKind::Insertion
        };
        let mut mark = RevisionMark::new(kind);
        mark.author.clone_from(&self.info.author);
        mark.date.clone_from(&self.info.date);
        mark.id.clone_from(&self.info.id);
        mark
    }
}
