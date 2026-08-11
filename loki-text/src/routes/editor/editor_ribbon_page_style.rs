// SPDX-License-Identifier: Apache-2.0

//! The Layout tab's entry point into the page style dialog (design section 3).
//!
//! The button opens the dialog on **the page style the caret's section uses**,
//! not on a fixed name: a document with a mirrored body and a plain front
//! matter has more than one, and opening the wrong one would silently edit a
//! part of the document the user is not looking at.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtRibbonIconButton, RibbonGroupSpec, estimate_group_metrics};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_ribbon_dialogs::dialog_label;
use super::editor_state::StyleDraft;
use super::editor_style_catalog::get_catalog_style;
use super::editor_style_editor::style_to_draft;
use super::editor_style_target::dialog_style_target;
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// The page style the caret sits in, falling back to the first catalogued one.
///
/// `None` only when there is no document, or when it has no page style at all —
/// in which case there is nothing for the dialog to edit and the button is
/// disabled rather than opening on an invented name.
#[must_use]
pub(super) fn caret_page_style(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor: &CursorState,
) -> Option<String> {
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;

    // The caret's paragraph index is flat across sections, so it has to be
    // converted before it can name one.
    let at_caret = cursor
        .focus
        .as_ref()
        .and_then(|pos| doc.flat_index_to_section_block(pos.paragraph_index))
        .and_then(|(section, _)| doc.sections.get(section))
        .and_then(|section| section.page_style.as_ref())
        .map(|id| id.as_str().to_string());

    at_caret.or_else(|| {
        doc.styles
            .page_styles
            .keys()
            .next()
            .map(|id| id.as_str().to_string())
    })
}

/// The Layout tab's **Page style…** group: the page dialog opener plus the
/// style catalog manager opener (§3a — the catalog editor's only entry point
/// since the dialogs patch rewired the Write tab's Paragraph button).
#[allow(clippy::too_many_arguments)]
pub(super) fn page_style_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    mut open: Signal<Option<String>>,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    priority: u8,
) -> RibbonGroupSpec {
    let target = caret_page_style(doc_state, &cursor_state.read());
    let has_target = target.is_some();
    let ds_manage = Arc::clone(doc_state);

    RibbonGroupSpec {
        metrics: estimate_group_metrics(priority, 2, true),
        partial: None,
        label: Some(fl!("ribbon-group-page-style")),
        aria_label: fl!("ribbon-group-page-style"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label: fl!("ribbon-page-style-dialog-aria"),
                is_active: open.read().is_some(),
                is_disabled: !has_target,
                on_click: move |_| {
                    if open.read().is_some() {
                        open.set(None);
                    } else {
                        open.set(target.clone());
                    }
                },
                {dialog_label(&fl!("ribbon-page-style-dialog-label"))}
            }
            // Opens the style catalog editor panel on the style at the caret,
            // resolved to a catalog id (and seeded when the definition is
            // missing) — the same target resolution the paragraph dialog uses,
            // so the panel never opens on a display key it cannot find.
            AtRibbonIconButton {
                aria_label: fl!("ribbon-manage-styles-aria"),
                is_active: editing_style_draft.read().is_some(),
                is_disabled: false,
                on_click: move |_| {
                    if editing_style_draft.read().is_some() {
                        editing_style_draft.set(None);
                        return;
                    }
                    let target = {
                        let ldoc_guard = loro_doc.read();
                        let cs = cursor_state.read();
                        match (ldoc_guard.as_ref(), cs.focus.as_ref()) {
                            (Some(ldoc), Some(focus)) => {
                                let target = dialog_style_target(
                                    &ds_manage, ldoc, focus.paragraph_index,
                                );
                                if target.as_ref().is_some_and(|t| t.seeded) {
                                    apply_mutation_and_relayout(&ds_manage, ldoc);
                                }
                                target
                            }
                            _ => None,
                        }
                    };
                    let Some(target) = target else { return };
                    if target.seeded {
                        post_mutation_sync(
                            &ds_manage, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                        );
                    }
                    let Some(style) = get_catalog_style(&ds_manage, &target.id) else {
                        return;
                    };
                    editing_style_draft.set(Some(style_to_draft(&style)));
                },
                {dialog_label(&fl!("ribbon-manage-styles-label"))}
            }
        },
    }
}
