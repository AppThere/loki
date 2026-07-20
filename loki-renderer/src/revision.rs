// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Active tracked-change **display mode** for the rendering layout path.
//!
//! Like the active spell state (`crate::spell`), the "Show Markup" view mode is
//! a per-view user setting rather than a document property: switching it must
//! not mutate the revision marks. Rather than thread it through every layout
//! call, the active [`RevisionDisplay`] is held here as app-internal ambient
//! state — the **Review** ribbon tab installs it via [`set_display`], and the
//! layout paths (`doc_page_source`, editor hit-testing) read it into
//! `LayoutOptions::revision_display`.
//!
//! The default is [`RevisionDisplay::AllMarkup`] (Word's "All Markup"), so
//! rendering is unchanged until the user picks Final / Original.

use std::sync::{PoisonError, RwLock};

use loki_layout::RevisionDisplay;

/// The active tracked-change display mode. Process-wide because it is one user
/// view setting shared by every open document and view.
static DISPLAY: RwLock<RevisionDisplay> = RwLock::new(RevisionDisplay::AllMarkup);

/// Installs the active tracked-change display mode (Review tab → Show Markup).
pub fn set_display(mode: RevisionDisplay) {
    *DISPLAY.write().unwrap_or_else(PoisonError::into_inner) = mode;
}

/// The current tracked-change display mode for `LayoutOptions::revision_display`.
pub fn display() -> RevisionDisplay {
    *DISPLAY.read().unwrap_or_else(PoisonError::into_inner)
}
