// SPDX-License-Identifier: Apache-2.0

//! Home tab ribbon content for the document editor.
//!
//! [`home_tab_content`] returns the `Element` passed to [`AtRibbon::tab_content`].
//! Separating it here keeps `editor_inner.rs` under the 300-line ceiling and
//! makes ribbon content easy to extend independently.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtRibbonGroup, AtRibbonIconButton};
use dioxus::prelude::*;
use loki_i18n::fl;
use loro::LoroDoc;

use crate::components::document_source::{DocumentState, apply_mutation_and_relayout};
use crate::editing::cursor::CursorState;

use super::editor_formatting;

/// Builds the Home tab ribbon content element.
///
/// Called once per render cycle from `EditorInner`.  The six formatting
/// signals drive the `is_active` state of each button.  Each button's
/// `on_click` calls the matching `editor_formatting::toggle_*` function and
/// then triggers a full relayout via `apply_mutation_and_relayout`.
///
/// Because [`Signal<T>`] is `Copy`, all signal parameters are copied freely
/// into closures.  One `Arc::clone` is made per button for `doc_state`.
pub(super) fn home_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<LoroDoc>>,
    cursor_state: Signal<CursorState>,
    bold_active: Signal<bool>,
    italic_active: Signal<bool>,
    underline_active: Signal<bool>,
    strikethrough_active: Signal<bool>,
    superscript_active: Signal<bool>,
    subscript_active: Signal<bool>,
) -> Element {
    // One Arc clone per button — cheap reference-count increment.
    let ds_bold = Arc::clone(doc_state);
    let ds_italic = Arc::clone(doc_state);
    let ds_underline = Arc::clone(doc_state);
    let ds_strike = Arc::clone(doc_state);
    let ds_super = Arc::clone(doc_state);
    let ds_sub = Arc::clone(doc_state);

    rsx! {
        AtRibbonGroup {
            label:      None,
            aria_label: fl!("ribbon-group-inline"),

            AtRibbonIconButton {
                icon_label: "B".to_string(),
                aria_label: fl!("ribbon-bold-aria"),
                is_active:  *bold_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_bold(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_bold, ldoc);
                    }
                },
            }

            AtRibbonIconButton {
                icon_label: "I".to_string(),
                aria_label: fl!("ribbon-italic-aria"),
                is_active:  *italic_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_italic(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_italic, ldoc);
                    }
                },
            }

            AtRibbonIconButton {
                icon_label: "U".to_string(),
                aria_label: fl!("ribbon-underline-aria"),
                is_active:  *underline_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_underline(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_underline, ldoc);
                    }
                },
            }

            AtRibbonIconButton {
                icon_label: "S\u{0336}".to_string(),
                aria_label: fl!("ribbon-strikethrough-aria"),
                is_active:  *strikethrough_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_strikethrough(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_strike, ldoc);
                    }
                },
            }

            AtRibbonIconButton {
                icon_label: "x\u{00B2}".to_string(),
                aria_label: fl!("ribbon-superscript-aria"),
                is_active:  *superscript_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_superscript(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_super, ldoc);
                    }
                },
            }

            AtRibbonIconButton {
                icon_label: "x\u{2082}".to_string(),
                aria_label: fl!("ribbon-subscript-aria"),
                is_active:  *subscript_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_subscript(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_sub, ldoc);
                    }
                },
            }
        }
    }
}
