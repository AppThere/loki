// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Active spell-check state for the rendering layout path.
//!
//! Spell checking is a single user-level setting (one active dictionary), and
//! the document is laid out for painting in [`crate::doc_page_source`]. Rather
//! than thread a checker through every layout call, the active [`SpellState`] is
//! held here as app-internal ambient state: the app installs it at startup (and
//! whenever the active dictionary changes) via [`set_active`], and the layout
//! paths read it into `LayoutOptions::spell` so misspelled words get squiggles.
//!
//! `None` (the default) disables squiggles — layout behaves exactly as before.
//! This lives in `loki-renderer` (not the app) so both the renderer's paint
//! layout and the editor's hit-test layout read the same source of truth.

use std::sync::{PoisonError, RwLock};

use loki_layout::SpellState;

/// The active spell state. Process-wide because the active dictionary is one
/// user setting shared by every open document and view.
static ACTIVE: RwLock<Option<SpellState>> = RwLock::new(None);

/// Installs (or clears, with `None`) the active spell state.
pub fn set_active(state: Option<SpellState>) {
    *ACTIVE.write().unwrap_or_else(PoisonError::into_inner) = state;
}

/// The current active spell state for `LayoutOptions::spell`, if any.
pub fn active() -> Option<SpellState> {
    ACTIVE
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}
