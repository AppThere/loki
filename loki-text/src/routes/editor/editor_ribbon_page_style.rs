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

use super::editor_ribbon_dialogs::dialog_label;
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

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

/// The Layout tab's **Page style…** group.
pub(super) fn page_style_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    mut open: Signal<Option<String>>,
    priority: u8,
) -> RibbonGroupSpec {
    let target = caret_page_style(doc_state, &cursor_state.read());
    let has_target = target.is_some();

    RibbonGroupSpec {
        metrics: estimate_group_metrics(priority, 1, true),
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
        },
    }
}
