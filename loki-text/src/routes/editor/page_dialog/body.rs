// SPDX-License-Identifier: Apache-2.0

//! Tab dispatch, the geometry read/write, and the measurement field the page
//! tabs share.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtField, DialogPosture, at_control_style, tokens};
use dioxus::prelude::*;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::set_page_style_geometry;
use loki_doc_model::style::catalog::StyleId;

use super::super::editor_defaults::PanelSettings;
use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::super::editor_style_editor::StyleEditorSync;
use super::tabs::{PageTab, unit_label};
use super::{
    PageDialogDraft, preview, tab_borders, tab_columns, tab_headfoot, tab_margins, tab_page,
};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// The draft signal every page field writes through.
pub(super) type PageDraft = Signal<Option<PageDialogDraft>>;

/// The geometry of the page style named `name`, if the document has one.
#[must_use]
pub(super) fn layout_for(doc_state: &Arc<Mutex<DocumentState>>, name: &str) -> Option<PageLayout> {
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;
    doc.styles
        .page_styles
        .get(&StyleId::new(name))
        .map(|style| style.layout.clone())
}

/// Writes `layout` onto the page style named `name`, returning whether it
/// landed.
pub(super) fn commit(
    doc_state: &Arc<Mutex<DocumentState>>,
    sync: &StyleEditorSync,
    name: &str,
    layout: &PageLayout,
) -> bool {
    let guard = sync.loro_doc.read();
    let Some(ldoc) = guard.as_ref() else {
        return false;
    };
    if set_page_style_geometry(ldoc, name, layout).is_err() {
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

/// Renders the active tab, with the paper preview docked beside it at Expanded.
pub(super) fn tab_body(
    tab: PageTab,
    doc_state: &Arc<Mutex<DocumentState>>,
    draft: PageDraft,
    posture: DialogPosture,
    settings: &PanelSettings,
) -> Element {
    let form = match tab {
        PageTab::Page => tab_page::body(doc_state, draft, posture, settings),
        PageTab::Margins => tab_margins::body(draft, posture, settings),
        PageTab::Columns => tab_columns::body(draft, posture, settings),
        PageTab::Header => tab_headfoot::body(draft, posture, settings, true),
        PageTab::Footer => tab_headfoot::body(draft, posture, settings, false),
        PageTab::Borders => tab_borders::body(draft, posture),
    };

    rsx! {
        div {
            style: "flex: 1; min-width: 0; display: flex; flex-direction: row;",
            {form}
            if posture.preview_docked {
                div {
                    style: format!(
                        "width: {w}px; flex-shrink: 0; border-left: 1px solid {border}; \
                         display: flex; flex-direction: column;",
                        w = tokens::DIALOG_PREVIEW_RAIL_PX,
                        border = tokens::COLOR_BORDER_CHROME,
                    ),
                    { preview::rail(draft, settings) }
                }
            }
        }
    }
}

/// The grid the page tabs lay out on.
#[must_use]
pub(super) fn grid_style(posture: DialogPosture) -> String {
    format!(
        "flex: 1; min-width: 0; display: grid; grid-template-columns: {cols}; \
         gap: {gy}px {gx}px; align-content: start; padding: {p}px; overflow-y: auto;",
        cols = if posture.full_screen {
            "1fr"
        } else {
            "1fr 1fr"
        },
        gy = tokens::SPACE_5,
        gx = tokens::SPACE_5,
        p = tokens::SPACE_5,
    )
}

/// A field spanning the whole grid.
#[must_use]
pub(super) fn span_all(posture: DialogPosture) -> String {
    if posture.full_screen {
        String::new()
    } else {
        "grid-column: 1 / -1;".to_string()
    }
}

/// A measurement field: caption, input carrying the display unit, footnote.
///
/// The buffer holds a **display-unit** number and the model holds points, so the
/// two are converted at the boundary rather than mixed: `MeasurementUnit::parse`
/// also honours an explicit suffix, which is what lets someone type `1in` into a
/// millimetre document without hunting for the unit setting.
#[allow(clippy::too_many_arguments)]
pub(super) fn measure_field(
    label: String,
    draft: PageDraft,
    posture: DialogPosture,
    settings: &PanelSettings,
    buffer: impl Fn(&PageDialogDraft) -> String,
    commit_value: impl Fn(&mut PageDialogDraft, Points) + 'static,
    set_buffer: impl Fn(&mut PageDialogDraft, String) + 'static,
    footnote: Option<String>,
    read_only: bool,
    extra_style: String,
) -> Element {
    let value = draft.read().as_ref().map(&buffer).unwrap_or_default();
    let unit = settings.unit;
    let suffix = unit_label(unit);

    rsx! {
        AtField {
            label,
            extra_style,
            disabled: read_only,
            control: rsx! {
                div {
                    style: at_control_style(posture.min_touch_px, "width: 100%;"),
                    input {
                        r#type: "text",
                        value: "{value}",
                        readonly: read_only,
                        style: format!(
                            "flex: 1; min-width: 0; background: transparent; border: none; \
                             font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_MD,
                            fg = if read_only {
                                tokens::COLOR_TEXT_ON_CHROME_SECONDARY
                            } else {
                                tokens::COLOR_TEXT_ON_CHROME
                            },
                        ),
                        oninput: move |evt| {
                            let mut draft = draft;
                            let mut next = draft.read().clone();
                            if let Some(d) = next.as_mut() {
                                let text = evt.value();
                                if let Some(points) = unit.parse(&text) {
                                    commit_value(d, points);
                                }
                                set_buffer(d, text);
                            }
                            draft.set(next);
                        },
                    }
                    span {
                        style: format!(
                            "flex-shrink: 0; font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        {suffix}
                    }
                }
            },
            footnote: match footnote {
                Some(text) => rsx! {
                    div {
                        style: format!(
                            "display: flex; align-items: center; gap: {gap}px; \
                             font-size: {fs}px; color: {fg};",
                            gap = tokens::SPACE_1,
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        span { "\u{21B3}" }
                        span { {text} }
                    }
                },
                None => rsx! {},
            },
        }
    }
}

/// A full-width uppercase section heading inside a page tab.
pub(super) fn section_heading(text: String, posture: DialogPosture) -> Element {
    rsx! {
        div {
            style: format!(
                "{span} font-size: {fs}px; font-weight: {fw}; letter-spacing: 0.04em; \
                 text-transform: uppercase; color: {fg};",
                span = span_all(posture),
                fs = tokens::FONT_SIZE_LABEL,
                fw = tokens::FONT_WEIGHT_SEMIBOLD,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
            ),
            {text}
        }
    }
}
