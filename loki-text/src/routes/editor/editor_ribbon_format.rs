// SPDX-License-Identifier: Apache-2.0

//! The Write tab's inline-formatting and paragraph-alignment ribbon groups.
//!
//! Extracted from `editor_ribbon` so `write_tab_content` stays under the
//! 300-line ceiling. Both groups apply their mutation to the live document,
//! relayout, and sync undo/redo — the same path keyboard shortcuts use.

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AtIcon, AtRibbonGroup, AtRibbonIconButton, LUCIDE_ALIGN_CENTER, LUCIDE_ALIGN_JUSTIFY,
    LUCIDE_ALIGN_LEFT, LUCIDE_ALIGN_RIGHT, LUCIDE_BOLD, LUCIDE_ITALIC, LUCIDE_STRIKETHROUGH,
    LUCIDE_SUBSCRIPT, LUCIDE_SUPERSCRIPT, LUCIDE_UNDERLINE,
};
use dioxus::prelude::*;
use loki_i18n::fl;
use loro::LoroDoc;

use super::editor_alignment::apply_alignment;
use super::editor_formatting;
use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// The six live signals + document handles every inline/alignment button needs.
/// Grouped so the two builder functions keep a small signature.
#[derive(Clone, Copy)]
pub(super) struct RibbonEditCtx {
    pub loro_doc: Signal<Option<LoroDoc>>,
    pub cursor_state: Signal<CursorState>,
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    pub can_undo: Signal<bool>,
    pub can_redo: Signal<bool>,
}

impl RibbonEditCtx {
    /// Relays out and syncs undo/redo after a button's mutation.
    fn finish(&self, doc_state: &Arc<Mutex<DocumentState>>, ldoc: &LoroDoc) {
        apply_mutation_and_relayout(doc_state, ldoc);
        post_mutation_sync(
            doc_state,
            self.loro_doc,
            self.cursor_state,
            self.undo_manager,
            self.can_undo,
            self.can_redo,
        );
    }
}

/// The six inline-format toggle states, driving each button's active styling.
#[derive(Clone, Copy)]
pub(super) struct InlineFormatState {
    pub bold: Signal<bool>,
    pub italic: Signal<bool>,
    pub underline: Signal<bool>,
    pub strikethrough: Signal<bool>,
    pub superscript: Signal<bool>,
    pub subscript: Signal<bool>,
}

/// The Inline formatting group (bold / italic / underline / strikethrough /
/// super / subscript).
pub(super) fn inline_format_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    ctx: RibbonEditCtx,
    state: InlineFormatState,
) -> Element {
    // One Arc clone per button — each on_click closure borrows its own.
    let ds_bold = Arc::clone(doc_state);
    let ds_italic = Arc::clone(doc_state);
    let ds_underline = Arc::clone(doc_state);
    let ds_strike = Arc::clone(doc_state);
    let ds_super = Arc::clone(doc_state);
    let ds_sub = Arc::clone(doc_state);
    let cursor = ctx.cursor_state;
    let loro = ctx.loro_doc;

    rsx! {
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-inline")),
            aria_label: fl!("ribbon-group-inline"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-bold-aria"),
                is_active:   *state.bold.read(),
                is_disabled: false,
                on_click: move |_| {
                    if let Some(ldoc) = loro.read().as_ref() {
                        let _ = editor_formatting::toggle_bold(ldoc, &cursor.read());
                        ctx.finish(&ds_bold, ldoc);
                    }
                },
                AtIcon { path_d: LUCIDE_BOLD.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-italic-aria"),
                is_active:   *state.italic.read(),
                is_disabled: false,
                on_click: move |_| {
                    if let Some(ldoc) = loro.read().as_ref() {
                        let _ = editor_formatting::toggle_italic(ldoc, &cursor.read());
                        ctx.finish(&ds_italic, ldoc);
                    }
                },
                AtIcon { path_d: LUCIDE_ITALIC.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-underline-aria"),
                is_active:   *state.underline.read(),
                is_disabled: false,
                on_click: move |_| {
                    if let Some(ldoc) = loro.read().as_ref() {
                        let _ = editor_formatting::toggle_underline(ldoc, &cursor.read());
                        ctx.finish(&ds_underline, ldoc);
                    }
                },
                AtIcon { path_d: LUCIDE_UNDERLINE.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-strikethrough-aria"),
                is_active:   *state.strikethrough.read(),
                is_disabled: false,
                on_click: move |_| {
                    if let Some(ldoc) = loro.read().as_ref() {
                        let _ = editor_formatting::toggle_strikethrough(ldoc, &cursor.read());
                        ctx.finish(&ds_strike, ldoc);
                    }
                },
                AtIcon { path_d: LUCIDE_STRIKETHROUGH.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-superscript-aria"),
                is_active:   *state.superscript.read(),
                is_disabled: false,
                on_click: move |_| {
                    if let Some(ldoc) = loro.read().as_ref() {
                        let _ = editor_formatting::toggle_superscript(ldoc, &cursor.read());
                        ctx.finish(&ds_super, ldoc);
                    }
                },
                AtIcon { path_d: LUCIDE_SUPERSCRIPT.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-subscript-aria"),
                is_active:   *state.subscript.read(),
                is_disabled: false,
                on_click: move |_| {
                    if let Some(ldoc) = loro.read().as_ref() {
                        let _ = editor_formatting::toggle_subscript(ldoc, &cursor.read());
                        ctx.finish(&ds_sub, ldoc);
                    }
                },
                AtIcon { path_d: LUCIDE_SUBSCRIPT.to_string() }
            }
        }
    }
}

/// One alignment button. `value` is the para-props alignment (`"Left"`, …).
fn align_button(
    doc_state: &Arc<Mutex<DocumentState>>,
    ctx: RibbonEditCtx,
    current: &str,
    value: &'static str,
    aria: String,
    icon: &'static str,
) -> Element {
    let ds = Arc::clone(doc_state);
    let loro = ctx.loro_doc;
    let cursor = ctx.cursor_state;
    rsx! {
        AtRibbonIconButton {
            aria_label:  aria,
            is_active:   current == value,
            is_disabled: false,
            on_click: move |_| {
                if let Some(ldoc) = loro.read().as_ref()
                    && apply_alignment(ldoc, &cursor.read(), value).is_ok()
                {
                    ctx.finish(&ds, ldoc);
                }
            },
            AtIcon { path_d: icon.to_string() }
        }
    }
}

/// The Paragraph alignment group (left / centre / right / justify).
pub(super) fn alignment_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    ctx: RibbonEditCtx,
    current_align: String,
) -> Element {
    rsx! {
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-alignment")),
            aria_label: fl!("ribbon-group-alignment"),

            {align_button(doc_state, ctx, &current_align, "Left", fl!("ribbon-align-left-aria"), LUCIDE_ALIGN_LEFT)}
            {align_button(doc_state, ctx, &current_align, "Center", fl!("ribbon-align-centre-aria"), LUCIDE_ALIGN_CENTER)}
            {align_button(doc_state, ctx, &current_align, "Right", fl!("ribbon-align-right-aria"), LUCIDE_ALIGN_RIGHT)}
            {align_button(doc_state, ctx, &current_align, "Justify", fl!("ribbon-align-justify-aria"), LUCIDE_ALIGN_JUSTIFY)}
        }
    }
}
