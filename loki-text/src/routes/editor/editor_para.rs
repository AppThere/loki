// SPDX-License-Identifier: Apache-2.0

//! Inline paragraph properties panel for the document editor.
//!
//! [`para_props_panel`] renders a strip of alignment buttons above the ribbon
//! when the paragraph-properties button in the ribbon is active.
//!
//! # Layout rationale
//!
//! `position: absolute` is confirmed unsupported in Blitz (dioxus-native).
//! The panel is therefore inline in the editor's flex column, above the ribbon.
//! When open it claims a fixed slice of vertical space; the canvas shrinks to
//! compensate (it uses `flex: 1` so Taffy redistributes).

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use appthere_ui::{
    AtIcon, AtRibbonGroup, AtRibbonIconButton, LUCIDE_ALIGN_CENTER, LUCIDE_ALIGN_JUSTIFY,
    LUCIDE_ALIGN_LEFT, LUCIDE_ALIGN_RIGHT,
};
use dioxus::prelude::*;
use loki_doc_model::{get_block_alignment, set_block_alignment};
use loki_i18n::fl;
use loro::LoroDoc;

use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use crate::routes::editor::editor_keydown_ctrl::post_mutation_sync;

/// Height of the open paragraph properties panel in CSS pixels.
pub(super) const PARA_PANEL_HEIGHT_PX: f32 = 56.0;

/// Renders the inline paragraph properties panel.
///
/// Plain function — no hooks.  All reactive state is passed in as signals.
/// Shows alignment buttons (Left / Center / Right / Justify) reflecting the
/// current paragraph's alignment.
///
/// # Touch target
///
/// Each alignment button satisfies the 44×44 px minimum (WCAG 2.5.8) via the
/// `AtRibbonIconButton` component.
#[allow(clippy::too_many_arguments)]
pub(super) fn para_props_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    mut is_para_props_open: Signal<bool>,
) -> Element {
    let current_alignment = {
        let cs = cursor_state.read();
        let ldoc = loro_doc.read();
        if let (Some(l), Some(focus)) = (ldoc.as_ref(), cs.focus.as_ref()) {
            get_block_alignment(l, focus.paragraph_index)
        } else {
            "Left".to_string()
        }
    };

    let ds_left = Arc::clone(&doc_state);
    let ds_center = Arc::clone(&doc_state);
    let ds_right = Arc::clone(&doc_state);
    let ds_justify = Arc::clone(&doc_state);

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; \
                 justify-content: space-between; \
                 height: {h}px; min-height: {h}px; max-height: {h}px; \
                 padding: 0 {p}px; background: {bg}; \
                 border-top: 1px solid {border}; flex-shrink: 0;",
                h      = PARA_PANEL_HEIGHT_PX,
                p      = tokens::SPACE_4,
                bg     = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
            ),
            div {
                style: "display: flex; flex-direction: row; align-items: center;",
                span {
                    style: format!(
                        "font-family: {ff}; font-size: {fs}px; font-weight: {fw}; \
                         color: {fg}; margin-right: {mr}px;",
                        ff = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_LABEL,
                        fw = tokens::FONT_WEIGHT_MEDIUM,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        mr = tokens::SPACE_4,
                    ),
                    { fl!("ribbon-para-props-heading") }
                }
                AtRibbonGroup {
                    label:      None,
                    aria_label: fl!("ribbon-group-paragraph"),

                    AtRibbonIconButton {
                        aria_label:  fl!("ribbon-align-left-aria"),
                        is_active:   current_alignment == "Left",
                        is_disabled: false,
                        on_click: move |_| {
                            let ldoc_guard = loro_doc.read();
                            if let Some(ldoc) = ldoc_guard.as_ref() {
                                let bi = cursor_state.read().focus.as_ref()
                                    .map(|f| f.paragraph_index)
                                    .unwrap_or(0);
                                let _ = set_block_alignment(ldoc, bi, "Left");
                                apply_mutation_and_relayout(&ds_left, ldoc);
                            }
                            post_mutation_sync(&ds_left, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                        },
                        AtIcon { path_d: LUCIDE_ALIGN_LEFT.to_string() }
                    }

                    AtRibbonIconButton {
                        aria_label:  fl!("ribbon-align-centre-aria"),
                        is_active:   current_alignment == "Center",
                        is_disabled: false,
                        on_click: move |_| {
                            let ldoc_guard = loro_doc.read();
                            if let Some(ldoc) = ldoc_guard.as_ref() {
                                let bi = cursor_state.read().focus.as_ref()
                                    .map(|f| f.paragraph_index)
                                    .unwrap_or(0);
                                let _ = set_block_alignment(ldoc, bi, "Center");
                                apply_mutation_and_relayout(&ds_center, ldoc);
                            }
                            post_mutation_sync(&ds_center, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                        },
                        AtIcon { path_d: LUCIDE_ALIGN_CENTER.to_string() }
                    }

                    AtRibbonIconButton {
                        aria_label:  fl!("ribbon-align-right-aria"),
                        is_active:   current_alignment == "Right",
                        is_disabled: false,
                        on_click: move |_| {
                            let ldoc_guard = loro_doc.read();
                            if let Some(ldoc) = ldoc_guard.as_ref() {
                                let bi = cursor_state.read().focus.as_ref()
                                    .map(|f| f.paragraph_index)
                                    .unwrap_or(0);
                                let _ = set_block_alignment(ldoc, bi, "Right");
                                apply_mutation_and_relayout(&ds_right, ldoc);
                            }
                            post_mutation_sync(&ds_right, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                        },
                        AtIcon { path_d: LUCIDE_ALIGN_RIGHT.to_string() }
                    }

                    AtRibbonIconButton {
                        aria_label:  fl!("ribbon-align-justify-aria"),
                        is_active:   current_alignment == "Justify",
                        is_disabled: false,
                        on_click: move |_| {
                            let ldoc_guard = loro_doc.read();
                            if let Some(ldoc) = ldoc_guard.as_ref() {
                                let bi = cursor_state.read().focus.as_ref()
                                    .map(|f| f.paragraph_index)
                                    .unwrap_or(0);
                                let _ = set_block_alignment(ldoc, bi, "Justify");
                                apply_mutation_and_relayout(&ds_justify, ldoc);
                            }
                            post_mutation_sync(&ds_justify, loro_doc, cursor_state, undo_manager, can_undo, can_redo);
                        },
                        AtIcon { path_d: LUCIDE_ALIGN_JUSTIFY.to_string() }
                    }
                }
            }
            button {
                style: format!(
                    "background: transparent; border: none; \
                     font-size: {fs}px; color: {fg}; cursor: pointer; \
                     padding: {p}px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    p  = tokens::SPACE_2,
                ),
                aria_label: "Close paragraph properties",
                onclick: move |_| { is_para_props_open.set(false); },
                "\u{2715}"
            }
        }
    }
}
