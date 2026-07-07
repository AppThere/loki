// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Intermediate model for DOCX tracked changes (`w:ins` / `w:del`).

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
