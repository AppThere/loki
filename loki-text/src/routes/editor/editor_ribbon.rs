// SPDX-License-Identifier: Apache-2.0

//! Home tab ribbon content for the document editor.
//!
//! [`home_tab_content`] returns the `Element` passed to [`AtRibbon::tab_content`].
//! Separating it here keeps `editor_inner.rs` under the 300-line ceiling and
//! makes ribbon content easy to extend independently.

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AtIcon, AtRibbonGroup, AtRibbonIconButton, AtRibbonSelect, LUCIDE_BOLD, LUCIDE_DOWNLOAD,
    LUCIDE_ITALIC, LUCIDE_PILCROW, LUCIDE_REDO, LUCIDE_SAVE, LUCIDE_STRIKETHROUGH,
    LUCIDE_SUBSCRIPT, LUCIDE_SUPERSCRIPT, LUCIDE_UNDERLINE, LUCIDE_UNDO,
};
use dioxus::prelude::*;
use loki_i18n::fl;
use loro::LoroDoc;

use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use crate::new_document::is_untitled;

use super::editor_formatting;
use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_save::save_document_to_path;
use super::editor_state::StyleDraft;
use super::editor_style_catalog::get_catalog_style;
use super::editor_style_editor::style_to_draft;

/// Builds the Home tab ribbon content element.
///
/// Called once per render cycle from `EditorInner`.  The six formatting
/// signals drive the `is_active` state of each button.  Each button's
/// `on_click` calls the matching `editor_formatting::toggle_*` function and
/// then triggers a full relayout via `apply_mutation_and_relayout`.
///
/// Because [`Signal<T>`] is `Copy`, all signal parameters are copied freely
/// into closures.  One `Arc::clone` is made per button for `doc_state`.
#[allow(clippy::too_many_arguments)]
pub(super) fn home_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<LoroDoc>>,
    cursor_state: Signal<CursorState>,
    mut undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    bold_active: Signal<bool>,
    italic_active: Signal<bool>,
    underline_active: Signal<bool>,
    strikethrough_active: Signal<bool>,
    superscript_active: Signal<bool>,
    subscript_active: Signal<bool>,
    current_style_name: String,
    mut is_style_picker_open: Signal<bool>,
    path_signal: Signal<String>,
    mut save_message: Signal<Option<String>>,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    save_as: Callback<()>,
    mut baseline_gen: Signal<u64>,
) -> Element {
    // One Arc clone per button — cheap reference-count increment.
    let ds_save = Arc::clone(doc_state);
    let ds_para = Arc::clone(doc_state);
    let current_style_name_para = current_style_name.clone();
    let ds_undo = Arc::clone(doc_state);
    let ds_redo = Arc::clone(doc_state);
    let ds_bold = Arc::clone(doc_state);
    let ds_italic = Arc::clone(doc_state);
    let ds_underline = Arc::clone(doc_state);
    let ds_strike = Arc::clone(doc_state);
    let ds_super = Arc::clone(doc_state);
    let ds_sub = Arc::clone(doc_state);

    rsx! {
        // ── Document group ────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      None,
            aria_label: fl!("ribbon-group-document"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-save-aria"),
                is_active:   false,
                is_disabled: false,
                on_click: move |_| {
                    let path = path_signal();
                    // An untitled document has no file yet — route to Save As.
                    if is_untitled(&path) {
                        save_as.call(());
                        return;
                    }
                    let msg = match save_document_to_path(&path, &ds_save) {
                        Ok(()) => {
                            // Mark the current generation as clean so the tab's
                            // unsaved-changes indicator clears.
                            baseline_gen.set(cursor_state.peek().document_generation);
                            fl!("editor-save-success")
                        }
                        Err(e) => fl!("editor-save-error", reason = e.to_string()),
                    };
                    save_message.set(Some(msg));
                },
                AtIcon { path_d: LUCIDE_SAVE.to_string() }
            }

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-save-as-aria"),
                is_active:   false,
                is_disabled: false,
                on_click: move |_| {
                    save_as.call(());
                },
                AtIcon { path_d: LUCIDE_DOWNLOAD.to_string() }
            }
        }

        // ── History group ─────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      None,
            aria_label: fl!("ribbon-group-history"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-undo-aria"),
                is_active:   false,
                is_disabled: !*can_undo.read(),
                on_click: move |_| {
                    {
                        let mut um_guard = undo_manager.write();
                        if let Some(um) = um_guard.as_mut() {
                            let _ = um.undo();
                        }
                    }
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        apply_mutation_and_relayout(&ds_undo, ldoc);
                    }
                    post_mutation_sync(&ds_undo, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_UNDO.to_string() }
            }

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-redo-aria"),
                is_active:   false,
                is_disabled: !*can_redo.read(),
                on_click: move |_| {
                    {
                        let mut um_guard = undo_manager.write();
                        if let Some(um) = um_guard.as_mut() {
                            let _ = um.redo();
                        }
                    }
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        apply_mutation_and_relayout(&ds_redo, ldoc);
                    }
                    post_mutation_sync(&ds_redo, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_REDO.to_string() }
            }
        }

        // ── Styles group ──────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      None,
            aria_label: fl!("ribbon-group-styles"),

            AtRibbonSelect {
                value:      current_style_name.clone(),
                aria_label: fl!("ribbon-style-select-aria"),
                is_open:    *is_style_picker_open.read(),
                on_open:    move |_| {
                    let currently_open = *is_style_picker_open.read();
                    is_style_picker_open.set(!currently_open);
                },
            }
        }

        // ── Paragraph group ───────────────────────────────────────────────────
        AtRibbonGroup {
            label:      None,
            aria_label: fl!("ribbon-group-paragraph"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-para-props-aria"),
                is_active:   editing_style_draft.read().is_some(),
                is_disabled: false,
                on_click: move |_| {
                    if editing_style_draft.read().is_some() {
                        editing_style_draft.set(None);
                        return;
                    }
                    let draft = get_catalog_style(&ds_para, &current_style_name_para)
                        .map(|s| style_to_draft(&s))
                        .unwrap_or_else(|| StyleDraft {
                            id: current_style_name_para.clone(),
                            name: current_style_name_para.clone(),
                            alignment: "Left".to_string(),
                            ..StyleDraft::default()
                        });
                    editing_style_draft.set(Some(draft));
                },
                AtIcon { path_d: LUCIDE_PILCROW.to_string() }
            }
        }

        // ── Inline formatting group ───────────────────────────────────────────
        AtRibbonGroup {
            label:      None,
            aria_label: fl!("ribbon-group-inline"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-bold-aria"),
                is_active:   *bold_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_bold(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_bold, ldoc);
                    }
                    post_mutation_sync(&ds_bold, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_BOLD.to_string() }
            }

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-italic-aria"),
                is_active:   *italic_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_italic(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_italic, ldoc);
                    }
                    post_mutation_sync(&ds_italic, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_ITALIC.to_string() }
            }

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-underline-aria"),
                is_active:   *underline_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_underline(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_underline, ldoc);
                    }
                    post_mutation_sync(&ds_underline, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_UNDERLINE.to_string() }
            }

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-strikethrough-aria"),
                is_active:   *strikethrough_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_strikethrough(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_strike, ldoc);
                    }
                    post_mutation_sync(&ds_strike, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_STRIKETHROUGH.to_string() }
            }

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-superscript-aria"),
                is_active:   *superscript_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_superscript(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_super, ldoc);
                    }
                    post_mutation_sync(&ds_super, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_SUPERSCRIPT.to_string() }
            }

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-subscript-aria"),
                is_active:   *subscript_active.read(),
                is_disabled: false,
                on_click: move |_| {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let _ = editor_formatting::toggle_subscript(ldoc, &cursor_state.read());
                        apply_mutation_and_relayout(&ds_sub, ldoc);
                    }
                    post_mutation_sync(&ds_sub, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_SUBSCRIPT.to_string() }
            }
        }
    }
}
