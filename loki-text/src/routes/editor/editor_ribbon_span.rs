// SPDX-License-Identifier: Apache-2.0

//! Format tab ribbon content — span-level (character) formatting.
//!
//! [`format_tab_content`] hosts the controls for the selected run: the
//! **Character style** select (the reusable span-level styles, opening the
//! docked picker panel), and the **Font colour** / **Highlight** picker
//! triggers (moved off the Write tab, which keeps only the core writing
//! controls). Each trigger toggles a docked panel above the ribbon.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtRibbonGroups, AtRibbonSelect, GroupMetrics, RibbonGroupSpec, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;
use loro::LoroDoc;

use super::editor_ribbon_color::{font_color_group, highlight_group};
use super::editor_state::ColorPickerTarget;
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// Builds the Format tab ribbon content element.
///
/// `open_color_picker` is the docked panel's open state, owned by
/// `EditorState`; the triggers toggle it and the panel itself is mounted by
/// `EditorInner` above the ribbon. `is_char_style_picker_open` gates the
/// character style picker panel the same way.
/// `span_format_dialog` opens the full span formatting dialog (design section
/// 2), which covers the same properties plus everything else a run carries —
/// the select and pickers stay because applying a style or a colour is a
/// one-click change and a dialog is not.
pub(super) fn format_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<LoroDoc>>,
    cursor_state: Signal<CursorState>,
    open_color_picker: Signal<Option<ColorPickerTarget>>,
    span_format_dialog: Signal<bool>,
    mut is_char_style_picker_open: Signal<bool>,
) -> Element {
    // Direct text colour / highlight at the caret — drives each trigger's
    // indicator bar (and the active swatch once the panel opens).
    let current_color = loro_doc
        .read()
        .as_ref()
        .and_then(|ldoc| super::editor_text_color::current_text_color(ldoc, &cursor_state.read()));
    let current_highlight = loro_doc.read().as_ref().and_then(|ldoc| {
        super::editor_highlight_color::current_highlight(ldoc, &cursor_state.read())
    });

    // The character style at the head of the selection, shown by display name
    // on the select — "None" when the run carries no reference.
    let current_char_style = loro_doc.read().as_ref().and_then(|ldoc| {
        super::span_dialog::char_style::read_char_style(ldoc, &cursor_state.read())
    });
    let char_style_value = current_char_style
        .map(|id| {
            super::editor_style_catalog::char_style_entries(doc_state)
                .into_iter()
                .find(|(sid, _)| *sid == id)
                .map_or(id, |(_, display)| display)
        })
        .unwrap_or_else(|| fl!("ribbon-char-style-none"));

    // A wide select, like the Write tab's paragraph style group (R-13e).
    let char_style = RibbonGroupSpec {
        metrics: GroupMetrics {
            priority: 3,
            full_px: tokens::RIBBON_SELECT_WIDTH_PX + 2.0 * tokens::SPACE_2,
            condensed_px: tokens::RIBBON_SELECT_WIDTH_CONDENSED_PX + 2.0 * tokens::SPACE_1,
            partial_px: None,
        },
        partial: None,
        label: Some(fl!("ribbon-group-char-style")),
        aria_label: fl!("ribbon-group-char-style"),
        content: rsx! {
            AtRibbonSelect {
                value:      char_style_value,
                aria_label: fl!("ribbon-char-style-select-aria"),
                is_open:    *is_char_style_picker_open.read(),
                on_open:    move |_| {
                    let currently_open = *is_char_style_picker_open.read();
                    is_char_style_picker_open.set(!currently_open);
                },
            }
        },
    };

    rsx! {
        AtRibbonGroups {
            overflow_aria_label: fl!("ribbon-overflow-aria"),
            groups: vec![
                // Kept full the longest: it is the whole tab in one button.
                super::editor_ribbon_dialogs::character_group(span_format_dialog, 4),
                char_style,
                font_color_group(current_color, open_color_picker, 1),
                highlight_group(current_highlight, open_color_picker, 0),
            ],
        }
    }
}
