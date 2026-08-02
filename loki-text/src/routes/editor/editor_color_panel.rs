// SPDX-License-Identifier: Apache-2.0

//! Docked colour-picker panel — hosted in `EditorInner`'s flex column above
//! the ribbon (the same posture as the style picker and publish panels),
//! opened by the Format tab's [`AtColorPickerTrigger`]s.
//!
//! Picks apply through the same mark-mutation + relayout path as every other
//! formatting control, are recorded into the session's recent-colour list,
//! and close the panel.
//!
//! [`AtColorPickerTrigger`]: appthere_ui::AtColorPickerTrigger

use std::sync::{Arc, Mutex};

use appthere_ui::{AtColorPickerLabels, AtColorPickerPanel};
use dioxus::prelude::*;
use loki_doc_model::MutationError;
use loki_i18n::fl;
use loro::LoroDoc;

use super::editor_highlight_color::{apply_highlight, current_highlight};
use super::editor_ribbon_color::{
    FONT_COLOR_PALETTE, highlight_swatches, preset_swatches, push_recent, recent_swatches,
};
use super::editor_ribbon_format::RibbonEditCtx;
use super::editor_state::ColorPickerTarget;
use super::editor_text_color::{apply_text_color, current_text_color};
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// Builds the docked picker panel for `target`.
pub(super) fn color_picker_panel(
    doc_state: &Arc<Mutex<DocumentState>>,
    target: ColorPickerTarget,
    mut open_picker: Signal<Option<ColorPickerTarget>>,
    ctx: RibbonEditCtx,
    recent_text_colors: Signal<Vec<String>>,
    recent_highlights: Signal<Vec<String>>,
) -> Element {
    let loro = ctx.loro_doc;
    let cursor = ctx.cursor_state;

    struct Target {
        current: Option<String>,
        swatches: Vec<appthere_ui::AtColorSwatch>,
        recent: Signal<Vec<String>>,
        show_custom: bool,
        title: String,
        clear: String,
        apply: fn(&LoroDoc, &CursorState, Option<&str>) -> Result<(), MutationError>,
    }

    let t = match target {
        ColorPickerTarget::Text => Target {
            current: loro
                .read()
                .as_ref()
                .and_then(|ldoc| current_text_color(ldoc, &cursor.read())),
            swatches: preset_swatches(FONT_COLOR_PALETTE),
            recent: recent_text_colors,
            show_custom: true,
            title: fl!("ribbon-group-font-color"),
            clear: fl!("ribbon-color-clear"),
            apply: apply_text_color,
        },
        ColorPickerTarget::Highlight => Target {
            current: loro
                .read()
                .as_ref()
                .and_then(|ldoc| current_highlight(ldoc, &cursor.read())),
            swatches: highlight_swatches(),
            recent: recent_highlights,
            // T5.3: a highlight may now be any colour, so the custom tab is
            // offered here as it always was for text.
            show_custom: true,
            title: fl!("ribbon-group-highlight"),
            clear: fl!("ribbon-highlight-clear"),
            apply: apply_highlight,
        },
    };

    let labels = AtColorPickerLabels {
        area: fl!("editor-color-area"),
        hue: fl!("editor-color-hue"),
        title: t.title,
        close: fl!("ribbon-color-close-aria"),
        clear: t.clear,
        recent_heading: fl!("ribbon-color-recent"),
        document_heading: fl!("ribbon-color-document"),
        custom_heading: fl!("ribbon-color-custom"),
        apply: fl!("ribbon-color-apply"),
    };
    // Every stored colour on every route is a `#RRGGBB`, so a swatch's fill is
    // its value. It was not before T5.3 — highlight recents held *variant names*
    // and needed a lookup, which the document group (added in T5.2's tail) then
    // fed hexes: `highlight_fill("#C0392B")` is `None`, so every document swatch
    // in the Highlight picker painted "transparent" and picking one wrote a hex
    // where a variant name was expected, applying nothing. One value shape
    // removes the lookup and the mismatch with it.
    let recent_list = recent_swatches(&t.recent.read());
    // Colours this document's styles define (T5.2). Read from the live document
    // rather than remembered, so it follows an edit that adds or removes one.
    let document_list = {
        let colors: Vec<String> = doc_state
            .lock()
            .ok()
            .and_then(|st| {
                st.document
                    .as_ref()
                    .map(|d| super::editor_doc_colors::document_colors(d))
            })
            .unwrap_or_default();
        recent_swatches(&colors)
    };
    let ds = Arc::clone(doc_state);
    let recent_sig = t.recent;
    let apply = t.apply;

    rsx! {
        AtColorPickerPanel {
            current_value: t.current,
            swatches: t.swatches,
            recent: recent_list,
            document: document_list,
            show_custom: t.show_custom,
            labels: labels,
            on_pick: move |value: Option<String>| {
                if let Some(ldoc) = loro.read().as_ref()
                    && apply(ldoc, &cursor.read(), value.as_deref()).is_ok()
                {
                    ctx.finish(&ds, ldoc);
                    if let Some(v) = value {
                        push_recent(recent_sig, v);
                    }
                }
                open_picker.set(None);
            },
            on_close: move |_| open_picker.set(None),
        }
    }
}
