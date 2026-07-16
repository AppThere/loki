// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Macro-present notice for the editor (macro spec §9.1, Phase 1).
//!
//! When an opened document carries a preserved macro payload
//! (`document.source.macros`), the editor shows a passive, non-modal infobar
//! stating that the document contains macros and that macros are **disabled**.
//! Phase 1 has no execution surface, so there is no "enable" action yet — the
//! notice is informational and dismissable. The trust/enable flow arrives in a
//! later phase (spec §2.3), at which point the infobar gains its action button.
//!
//! Detection reads the retained [`DocumentState::document`] provenance; the
//! macro payload lives outside the Loro CRDT (it is preserved on the document
//! source), so it is available here without touching the edit state.

use std::sync::{Arc, Mutex};

use appthere_ui::AtInfobar;
use dioxus::prelude::*;
use loki_i18n::fl;

use crate::editing::state::DocumentState;

/// Returns `true` if the currently-loaded document carries a preserved macro
/// payload. Non-blocking: a poisoned or contended lock reports "no macros this
/// frame" rather than stalling the UI thread; the next render re-reads.
pub(super) fn macros_present(doc_state: &Arc<Mutex<DocumentState>>) -> bool {
    doc_state
        .try_lock()
        .ok()
        .and_then(|s| {
            s.document
                .as_ref()
                .and_then(|d| d.source.as_ref())
                .map(|src| src.macros.as_ref().is_some_and(|m| !m.is_empty()))
        })
        .unwrap_or(false)
}

/// The passive "this document contains macros — macros are disabled" infobar.
///
/// Always mounted (so its dismiss state is stable across re-renders); renders
/// nothing unless the document carries macros and the user has not dismissed
/// the notice this session. Dismissing hides it until the document is reopened.
///
/// Touch targets: the infobar's dismiss control meets the 44×44 logical-pixel
/// minimum (WCAG 2.5.8) via [`AtInfobar`]'s control sizing.
#[component]
pub(super) fn MacroNoticeBar(present: bool) -> Element {
    let mut dismissed = use_signal(|| false);

    if !present || dismissed() {
        return rsx! {};
    }

    rsx! {
        AtInfobar {
            message: fl!("macros-infobar-disabled"),
            dismiss_label: fl!("macros-infobar-dismiss"),
            on_dismiss: move |()| dismissed.set(true),
        }
    }
}
