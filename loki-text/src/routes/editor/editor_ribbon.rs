// SPDX-License-Identifier: Apache-2.0

//! Write tab ribbon content for the document editor (was "Home", Spec 04 D1).
//!
//! [`write_tab_content`] returns the `Element` passed to [`AtRibbon::tab_content`].
//! Separating it here keeps `editor_inner.rs` under the 300-line ceiling.

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AtIcon, AtRibbonGroup, AtRibbonIconButton, AtRibbonSelect, LUCIDE_DOWNLOAD,
    LUCIDE_LAYOUT_TEMPLATE, LUCIDE_PILCROW, LUCIDE_REDO, LUCIDE_SAVE, LUCIDE_UNDO,
};
use dioxus::prelude::*;
use loki_i18n::fl;
use loro::LoroDoc;

use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_state::StyleDraft;
use super::editor_style_catalog::get_catalog_style;
use super::editor_style_editor::style_to_draft;

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
    mut save_request: Signal<u32>,
    is_dirty: Signal<bool>,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    save_as: Callback<()>,
    save_as_template: Callback<()>,
) -> Element {
    // One Arc clone per button — cheap reference-count increment.
    let ds_para = Arc::clone(doc_state);
    let current_style_name_para = current_style_name.clone();
    let ds_undo = Arc::clone(doc_state);
    let ds_redo = Arc::clone(doc_state);

    // The inline-formatting and alignment groups are extracted to
    // `editor_ribbon_format` (ceiling). They share these live handles + states.
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
    // Alignment of the caret's paragraph, for the alignment group's active state.
    let current_align = loro_doc
        .read()
        .as_ref()
        .map(|ldoc| super::editor_alignment::current_alignment(ldoc, &cursor_state.read()))
        .unwrap_or_else(|| "Left".to_string());

    rsx! {
        // ── Document group ────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-document")),
            aria_label: fl!("ribbon-group-document"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-save-aria"),
                is_active:   false,
                // Disabled when clean (plan 4b.3); untitled reads as dirty.
                is_disabled: !is_dirty(),
                on_click: move |_| {
                    // Route through the shared save handler (the Ctrl+S effect
                    // in `EditorInner`), which owns the untitled→Save-As
                    // routing, the clean baseline, the status message, and
                    // post-save history compaction.
                    let next = save_request.peek().wrapping_add(1);
                    save_request.set(next);
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

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-save-as-template-aria"),
                is_active:   false,
                is_disabled: false,
                on_click: move |_| {
                    save_as_template.call(());
                },
                AtIcon { path_d: LUCIDE_LAYOUT_TEMPLATE.to_string() }
            }
        }

        // ── History group ─────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-history")),
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
            label:      Some(fl!("ribbon-group-styles")),
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
            label:      Some(fl!("ribbon-group-paragraph")),
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

        // ── Font, inline-formatting, and alignment groups (editor_ribbon_format) ─
        {super::editor_ribbon_format::font_group(doc_state, edit_ctx)}

        {super::editor_ribbon_format::inline_format_group(doc_state, edit_ctx, inline_state)}

        {super::editor_ribbon_format::alignment_group(doc_state, edit_ctx, current_align)}
    }
}
