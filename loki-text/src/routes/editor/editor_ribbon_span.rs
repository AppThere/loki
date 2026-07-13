// SPDX-License-Identifier: Apache-2.0

//! Format tab ribbon content — span-level (character) colour formatting.
//!
//! [`format_tab_content`] hosts the controls that colour the selected text:
//! the **Font colour** and **Highlight** picker triggers (moved off the Write
//! tab, which keeps only the core writing controls). Each trigger toggles the
//! docked picker panel (`editor_color_panel`) above the ribbon.

use appthere_ui::AtRibbonGroups;
use dioxus::prelude::*;
use loki_i18n::fl;
use loro::LoroDoc;

use super::editor_ribbon_color::{font_color_group, highlight_group};
use super::editor_state::ColorPickerTarget;
use crate::editing::cursor::CursorState;

/// Builds the Format tab ribbon content element.
///
/// `open_color_picker` is the docked panel's open state, owned by
/// `EditorState`; the triggers toggle it and the panel itself is mounted by
/// `EditorInner` above the ribbon.
pub(super) fn format_tab_content(
    loro_doc: Signal<Option<LoroDoc>>,
    cursor_state: Signal<CursorState>,
    open_color_picker: Signal<Option<ColorPickerTarget>>,
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

    rsx! {
        AtRibbonGroups {
            overflow_aria_label: fl!("ribbon-overflow-aria"),
            groups: vec![
                font_color_group(current_color, open_color_picker, 1),
                highlight_group(current_highlight, open_color_picker, 0),
            ],
        }
    }
}
