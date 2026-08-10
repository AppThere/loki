// SPDX-License-Identifier: Apache-2.0

//! Write tab ribbon content for the document editor (was "Home", Spec 04 D1).
//!
//! [`write_tab_content`] returns the `Element` passed to [`AtRibbon::tab_content`].
//! Separating it here keeps `editor_inner.rs` under the 300-line ceiling.

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AtIcon, AtRibbonGroups, AtRibbonIconButton, AtRibbonSelect, GroupMetrics, LUCIDE_PILCROW,
    LUCIDE_REDO, LUCIDE_UNDO, RibbonGroupSpec, estimate_group_metrics, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;
use loro::LoroDoc;

use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_style_target::dialog_style_target;

/// Builds the Write tab ribbon content element.
///
/// Called once per render cycle from `EditorInner`.  The six formatting
/// signals drive the `is_active` state of each button.  Each button's
/// `on_click` calls the matching `editor_formatting::toggle_*` function and
/// then triggers a full relayout via `apply_mutation_and_relayout`.
///
/// Because [`Signal<T>`] is `Copy`, all signal parameters are copied freely
/// into closures.  One `Arc::clone` is made per button for `doc_state`.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_tab_content(
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
    save_request: Signal<u32>,
    is_dirty: Signal<bool>,
    // The paragraph style open in the tabbed style dialog (Spec 05 M2/M6).
    mut paragraph_style_dialog: Signal<Option<String>>,
    // The Document group's callbacks + a recents snapshot (usage audit §4).
    document_actions: super::editor_ribbon_document::DocumentGroupActions,
    recents: Vec<(String, String)>,
) -> Element {
    // One Arc clone per button — cheap reference-count increment.
    let ds_undo = Arc::clone(doc_state);
    let ds_redo = Arc::clone(doc_state);
    let ds_para = Arc::clone(doc_state);

    // The inline-formatting group is extracted to `editor_ribbon_format`
    // (ceiling). It shares these live handles + states.
    let edit_ctx = super::editor_ribbon_format::RibbonEditCtx {
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    };
    let inline_state = super::editor_ribbon_format::InlineFormatState {
        bold: bold_active,
        italic: italic_active,
        underline: underline_active,
        strikethrough: strikethrough_active,
        superscript: superscript_active,
        subscript: subscript_active,
    };

    // Collapse priorities (higher = kept full longer, Spec 04 M3 §7): the core
    // editing controls (Inline, Styles) stay full the longest. Font/highlight
    // colour live on the Format tab; paragraph alignment lives in the
    // paragraph style editor.
    let document = super::editor_ribbon_document::document_group(
        document_actions,
        recents,
        is_dirty(),
        save_request,
        4,
    );

    let history = RibbonGroupSpec {
        metrics: estimate_group_metrics(3, 2, true),
        label: Some(fl!("ribbon-group-history")),
        aria_label: fl!("ribbon-group-history"),
        content: rsx! {
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
        },
    };

    let styles = RibbonGroupSpec {
        // A wide select, not icon buttons — size from the select-width tokens
        // (R-13e: the select itself narrows in the condensed state).
        metrics: GroupMetrics {
            priority: 5,
            full_px: tokens::RIBBON_SELECT_WIDTH_PX + 2.0 * tokens::SPACE_2,
            condensed_px: tokens::RIBBON_SELECT_WIDTH_CONDENSED_PX + 2.0 * tokens::SPACE_1,
        },
        label: Some(fl!("ribbon-group-styles")),
        aria_label: fl!("ribbon-group-styles"),
        content: rsx! {
            AtRibbonSelect {
                value:      current_style_name.clone(),
                aria_label: fl!("ribbon-style-select-aria"),
                is_open:    *is_style_picker_open.read(),
                on_open:    move |_| {
                    let currently_open = *is_style_picker_open.read();
                    is_style_picker_open.set(!currently_open);
                },
            }
        },
    };

    let paragraph = RibbonGroupSpec {
        metrics: estimate_group_metrics(2, 1, true),
        label: Some(fl!("ribbon-group-paragraph")),
        aria_label: fl!("ribbon-group-paragraph"),
        content: rsx! {
            // Opens the tabbed paragraph style editor on the style the block
            // at the cursor resolves through. The dialog reads the catalog
            // itself, so this passes a catalog id rather than a draft — and the
            // id must be a *catalog* id, not `get_block_style_name`'s display
            // key, or the dialog's missing-style guard closes it silently
            // (`dialog_style_target` maps one to the other, seeding the
            // definition when the catalog lacks it).
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-para-props-aria"),
                is_active:   paragraph_style_dialog.read().is_some(),
                is_disabled: false,
                on_click: move |_| {
                    if paragraph_style_dialog.read().is_some() {
                        paragraph_style_dialog.set(None);
                        return;
                    }
                    let target = {
                        let ldoc_guard = loro_doc.read();
                        let cs = cursor_state.read();
                        match (ldoc_guard.as_ref(), cs.focus.as_ref()) {
                            (Some(ldoc), Some(focus)) => {
                                let target =
                                    dialog_style_target(&ds_para, ldoc, focus.paragraph_index);
                                if target.as_ref().is_some_and(|t| t.seeded) {
                                    apply_mutation_and_relayout(&ds_para, ldoc);
                                }
                                target
                            }
                            _ => None,
                        }
                    };
                    let Some(target) = target else { return };
                    if target.seeded {
                        post_mutation_sync(
                            &ds_para, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                        );
                    }
                    paragraph_style_dialog.set(Some(target.id));
                },
                AtIcon { path_d: LUCIDE_PILCROW.to_string() }
            }
        },
    };

    rsx! {
        AtRibbonGroups {
            overflow_aria_label: fl!("ribbon-overflow-aria"),
            groups: vec![
                document,
                history,
                styles,
                paragraph,
                super::editor_ribbon_format::inline_format_group(doc_state, edit_ctx, inline_state, 8),
            ],
        }
    }
}
