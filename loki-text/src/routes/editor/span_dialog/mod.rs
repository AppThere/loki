// SPDX-License-Identifier: Apache-2.0

//! The span-level (character) formatting dialog (design section 2).
//!
//! Direct formatting on the selected run. Character styles resolve their own
//! chain, so the same provenance line appears here — plus a **fourth** state,
//! *direct formatting*, which sits above every style and is the first thing
//! "Clear direct formatting" removes (design note 09).

mod body;
pub(in crate::routes::editor) mod char_style;
mod marks;
mod selection;
mod tab_effects;
mod tab_font;
mod tab_highlight;
mod tabs;

use std::sync::{Arc, Mutex};

use appthere_ui::responsive::use_breakpoint;
use appthere_ui::{
    AtDialogButton, AtDialogShell, AtDialogTabStrip, DialogPosture, DialogWidth, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_insert_sync::InsertLinkSync;
use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use char_style::{apply_char_style, read_char_style};
use marks::{SpanMarks, apply_marks, clear_direct, read_marks};
use tabs::SpanTab;

/// The dialog's working state: the marks as read, and as edited.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SpanDraft {
    /// The marks as they will be committed.
    pub marks: SpanMarks,
    /// The marks as the selection carried them when the dialog opened, so only
    /// what changed is written and Cancel is a discard.
    pub original: SpanMarks,
    /// The character-style reference staged for the selection. A style level,
    /// not a mark: Clear direct formatting leaves it alone (design note 09).
    pub char_style: Option<String>,
    /// The reference as the selection carried it when the dialog opened.
    pub original_char_style: Option<String>,
    /// Buffer for the size box.
    pub size_buffer: String,
    /// Buffer for the letter-spacing box.
    pub spacing_buffer: String,
    /// Characters in the selection, for the header summary.
    pub selection_chars: usize,
}

impl SpanDraft {
    /// Opens a draft over the marks currently on the selection.
    #[must_use]
    pub fn new(marks: SpanMarks, char_style: Option<String>, selection_chars: usize) -> Self {
        let size_buffer = marks.font_size_pt.map(format_number).unwrap_or_default();
        let spacing_buffer = marks
            .letter_spacing_pt
            .map(format_number)
            .unwrap_or_default();
        Self {
            original: marks.clone(),
            marks,
            original_char_style: char_style.clone(),
            char_style,
            size_buffer,
            spacing_buffer,
            selection_chars,
        }
    }

    /// `true` when something is staged.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.marks != self.original || self.char_style != self.original_char_style
    }
}

/// Renders a measurement without trailing zeros: `10` rather than `10.00`.
#[must_use]
pub(super) fn format_number(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Props for [`SpanFormatDialog`].
#[derive(Clone, Props)]
pub(super) struct SpanFormatDialogProps {
    /// Shared document state.
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    /// `true` while the dialog is open.
    pub(super) open: Signal<bool>,
    /// Font families enumerated on this device.
    pub(super) font_families: std::rc::Rc<Vec<String>>,
    /// Loro / undo plumbing.
    pub(super) sync: InsertLinkSync,
}

impl PartialEq for SpanFormatDialogProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.open == other.open
            && std::rc::Rc::ptr_eq(&self.font_families, &other.font_families)
            && self.sync == other.sync
    }
}

/// Renders the character-formatting dialog.
// PascalCase for rsx; `#[component]` cannot derive the props comparison.
#[allow(non_snake_case)]
pub(super) fn SpanFormatDialog(props: SpanFormatDialogProps) -> Element {
    let SpanFormatDialogProps {
        doc_state,
        mut open,
        font_families,
        sync,
    } = props;
    let posture = DialogPosture::for_breakpoint(use_breakpoint());

    let mut draft = use_signal(|| {
        let guard = sync.loro_doc.read();
        let cursor = sync.cursor_state.read().clone();
        guard.as_ref().map(|ldoc| {
            SpanDraft::new(
                read_marks(ldoc, &cursor),
                read_char_style(ldoc, &cursor),
                selection::selection_len(&doc_state, &sync),
            )
        })
    });
    let mut active_tab = use_signal(|| SpanTab::Font);
    let menu_open = use_signal(|| false);

    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let tab = *active_tab.read();
    let dirty = current.is_dirty();
    let has_direct = !current.marks.is_empty();
    let ds_apply = Arc::clone(&doc_state);
    let ds_clear = Arc::clone(&doc_state);
    let styles = body::style_context(&doc_state, &sync);

    rsx! {
        AtDialogShell {
            title: fl!("span-dialog-title"),
            subtitle: rsx! {
                div {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    {
                        tabs::selection_summary(
                            current.selection_chars,
                            styles.char_style.as_deref(),
                            &current.marks,
                        )
                    }
                }
            },
            close_aria_label: fl!("span-dialog-close-aria"),
            width: DialogWidth::Narrow,
            on_close: move |_| open.set(false),

            tabs: rsx! {
                AtDialogTabStrip {
                    labels: SpanTab::labels(),
                    active: tab.index(),
                    inline_at_medium: SpanTab::INLINE_AT_MEDIUM,
                    menu_open,
                    more_label: fl!("span-dialog-tabs-more"),
                    on_select: move |idx: usize| active_tab.set(SpanTab::from_index(idx)),
                }
            },

            body: rsx! { { body::tab_body(tab, draft, posture, &styles, font_families) } },

            footer: rsx! {
                AtDialogButton {
                    label: fl!("span-dialog-clear-direct"),
                    tertiary: true,
                    // Nothing to clear on a run with no marks; a live button
                    // there would promise an effect it cannot have.
                    disabled: !has_direct,
                    min_touch_px: posture.min_touch_px,
                    on_click: move |_| {
                        if run_clear(&ds_clear, &sync) {
                            let guard = sync.loro_doc.read();
                            let cursor = sync.cursor_state.read().clone();
                            let refreshed = guard.as_ref().map(|ldoc| {
                                SpanDraft::new(
                                    read_marks(ldoc, &cursor),
                                    read_char_style(ldoc, &cursor),
                                    selection::selection_len(&doc_state, &sync),
                                )
                            });
                            drop(guard);
                            draft.set(refreshed);
                        }
                    },
                }
                div {
                    style: format!(
                        "display: flex; flex-direction: {dir}; gap: {gap}px;",
                        dir = if posture.stack_footer { "column-reverse" } else { "row" },
                        gap = tokens::SPACE_2,
                    ),
                    AtDialogButton {
                        label: fl!("span-dialog-cancel"),
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| open.set(false),
                    }
                    AtDialogButton {
                        label: fl!("span-dialog-apply"),
                        primary: true,
                        disabled: !dirty,
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| {
                            let Some(d) = draft.read().clone() else { return };
                            if run_apply(&ds_apply, &sync, &d) {
                                open.set(false);
                            }
                        },
                    }
                }
            },
        }
    }
}

/// Writes the staged marks over the selection.
fn run_apply(
    doc_state: &Arc<Mutex<DocumentState>>,
    sync: &InsertLinkSync,
    draft: &SpanDraft,
) -> bool {
    let guard = sync.loro_doc.read();
    let Some(ldoc) = guard.as_ref() else {
        return false;
    };
    let cursor = sync.cursor_state.read().clone();
    if apply_marks(ldoc, &cursor, &draft.original, &draft.marks).is_err() {
        return false;
    }
    if apply_char_style(ldoc, &cursor, &draft.original_char_style, &draft.char_style).is_err() {
        return false;
    }
    apply_mutation_and_relayout(doc_state, ldoc);
    drop(guard);
    post_mutation_sync(
        doc_state,
        sync.loro_doc,
        sync.cursor_state,
        sync.undo_manager,
        sync.can_undo,
        sync.can_redo,
    );
    true
}

/// Removes every direct-formatting mark from the selection.
fn run_clear(doc_state: &Arc<Mutex<DocumentState>>, sync: &InsertLinkSync) -> bool {
    let guard = sync.loro_doc.read();
    let Some(ldoc) = guard.as_ref() else {
        return false;
    };
    let cursor = sync.cursor_state.read().clone();
    if clear_direct(ldoc, &cursor).is_err() {
        return false;
    }
    apply_mutation_and_relayout(doc_state, ldoc);
    drop(guard);
    post_mutation_sync(
        doc_state,
        sync.loro_doc,
        sync.cursor_state,
        sync.undo_manager,
        sync.can_undo,
        sync.can_redo,
    );
    true
}
