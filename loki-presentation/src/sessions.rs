// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! In-memory editing sessions for inactive presentation tabs.
//!
//! When the user switches away from a presentation tab, `EditorInner` stashes
//! the live editable state here instead of discarding it; switching back
//! restores the session so unsaved edits survive tab switches (audit F1
//! residual / plan 4b.6). Closing a tab drops its session (see
//! `routes/shell.rs`).
//!
//! The map is provided as `Signal<DocSessions>` in Dioxus context at the
//! [`crate::app::App`] root. Sessions exist only for *inactive* tabs — the
//! active tab's state lives in the editor signals.

use std::collections::HashMap;

use loki_presentation_model::Presentation;

/// Live editing state for one open-but-inactive presentation.
pub struct DocSession {
    /// The editable presentation — holds all unsaved edits.
    pub doc: Presentation,
    /// Index of the slide that was active at stash time.
    pub active_idx: usize,
    /// Whether the presentation had unsaved edits at stash time.
    pub dirty: bool,
}

/// Open-but-inactive presentation sessions, keyed by the serialised file token.
pub type DocSessions = HashMap<String, DocSession>;
