// SPDX-License-Identifier: Apache-2.0

//! The span dialog's tab bodies.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use appthere_ui::{AtCheckRow, AtField, AtProvenanceLine, DialogPosture, at_control_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_insert_sync::InsertLinkSync;
use super::SpanDraft;
use super::tabs::{SpanLevel, SpanTab, level_of};
use crate::editing::state::DocumentState;

/// The draft signal every span control writes through.
pub(super) type SpanDraftSignal = Signal<Option<SpanDraft>>;

/// The styles in play under the selection — the two inherited levels the
/// provenance line names — plus the catalog's character styles, which the
/// Font tab offers for applying.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct StyleContext {
    /// The run's character style (display name), if it has one.
    pub char_style: Option<String>,
    /// The enclosing paragraph's style.
    pub para_style: Option<String>,
    /// Every character style the catalog defines, as `(id, display name)` —
    /// what the Character style picker lists.
    pub char_styles: Vec<(String, String)>,
}

/// The character and paragraph styles under the selection.
#[must_use]
pub(super) fn style_context(
    doc_state: &Arc<Mutex<DocumentState>>,
    sync: &InsertLinkSync,
) -> StyleContext {
    let cursor = sync.cursor_state.read().clone();
    // The reference travels as a span mark, so it is read where the marks are.
    let char_style_id = {
        let guard = sync.loro_doc.read();
        guard
            .as_ref()
            .and_then(|ldoc| super::char_style::read_char_style(ldoc, &cursor))
    };
    let Some(focus) = cursor.focus.as_ref() else {
        return StyleContext::default();
    };
    let char_styles = super::super::editor_style_catalog::char_style_entries(doc_state);
    let Ok(state) = doc_state.lock() else {
        return StyleContext::default();
    };
    let Some(doc) = state.document.as_ref() else {
        return StyleContext::default();
    };
    // The provenance lines name the style by its display name.
    let char_style = char_style_id.map(|id| {
        char_styles
            .iter()
            .find(|(sid, _)| *sid == id)
            .map_or_else(|| id.clone(), |(_, display)| display.clone())
    });
    let para_style = doc
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .nth(focus.paragraph_index)
        .and_then(block_style_name);
    StyleContext {
        char_style,
        para_style,
        char_styles,
    }
}

/// The style name a block carries, if any.
fn block_style_name(block: &loki_doc_model::content::block::Block) -> Option<String> {
    use loki_doc_model::content::block::Block;
    match block {
        Block::StyledPara(para) => para.style_id.as_ref().map(|s| s.as_str().to_string()),
        Block::Heading(level, _, _) => Some(format!("Heading {level}")),
        _ => None,
    }
}

/// Renders the active tab's body.
pub(super) fn tab_body(
    tab: SpanTab,
    draft: SpanDraftSignal,
    posture: DialogPosture,
    styles: &StyleContext,
    font_families: Rc<Vec<String>>,
) -> Element {
    match tab {
        SpanTab::Font => super::tab_font::font(draft, posture, styles, font_families),
        SpanTab::Effects => super::tab_effects::effects(draft, posture, styles),
        SpanTab::Position => super::tab_effects::position(draft, posture, styles),
        SpanTab::Highlight => super::tab_highlight::highlight(draft, posture, styles),
        SpanTab::Language => super::tab_highlight::language(draft, posture, styles),
    }
}

/// The grid every span tab lays out on.
pub(super) fn grid(posture: DialogPosture) -> String {
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
pub(super) fn span_all(posture: DialogPosture) -> String {
    if posture.full_screen {
        String::new()
    } else {
        "grid-column: 1 / -1;".to_string()
    }
}

/// The provenance line for one span property.
pub(super) fn line(
    marked: bool,
    value: Option<String>,
    styles: &StyleContext,
    posture: DialogPosture,
    mut draft: SpanDraftSignal,
    reset: impl Fn(&mut SpanDraft) + 'static,
) -> Element {
    let level = level_of(
        marked,
        styles.char_style.as_deref(),
        styles.para_style.as_deref(),
    );
    let source = match level {
        SpanLevel::CharacterStyle => styles.char_style.clone(),
        SpanLevel::ParagraphStyle => styles.para_style.clone(),
        _ => None,
    };
    let text = level.text(value.as_deref(), source.as_deref());

    rsx! {
        AtProvenanceLine {
            kind: level.kind(),
            text,
            min_touch_px: posture.min_touch_px,
            reset_label: level.is_resettable().then(|| fl!("span-dialog-reset")),
            on_reset: EventHandler::new(move |()| {
                let mut next = draft.read().clone();
                if let Some(d) = next.as_mut() {
                    reset(d);
                }
                draft.set(next);
            }),
        }
    }
}

/// Applies an edit to the draft.
pub(super) fn edit(mut draft: SpanDraftSignal, change: impl Fn(&mut SpanDraft)) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        change(d);
    }
    draft.set(next);
}

/// A tri-state toggle over an `Option<bool>` mark.
///
/// Three states, not two: **on**, **off**, and *not set here* — and the third is
/// not the same as off. A run whose character style is bold and which sets
/// `bold = false` directly is not-bold *because it says so*, which is what the
/// accent dot and the Reset are reporting.
pub(super) fn tri_toggle(
    label: String,
    current: Option<bool>,
    posture: DialogPosture,
    draft: SpanDraftSignal,
    set: impl Fn(&mut SpanDraft, Option<bool>) + Copy + 'static,
) -> Element {
    rsx! {
        AtCheckRow {
            checked: current.unwrap_or(false),
            min_touch_px: posture.min_touch_px,
            aria_label: label.clone(),
            label: rsx! { {label} },
            on_toggle: move |v: bool| edit(draft, move |d| set(d, Some(v))),
        }
    }
}

/// A numeric field with a unit suffix and a supplied provenance line.
pub(super) fn number_field(
    label: String,
    value: String,
    suffix: String,
    posture: DialogPosture,
    draft: SpanDraftSignal,
    commit: impl Fn(&mut SpanDraft, String) + 'static,
    footnote: Element,
) -> Element {
    let commit = std::rc::Rc::new(commit);
    rsx! {
        AtField {
            label,
            control: rsx! {
                div {
                    style: at_control_style(posture.min_touch_px, "width: 100%;"),
                    input {
                        r#type: "text",
                        value: "{value}",
                        style: format!(
                            "flex: 1; min-width: 0; background: transparent; border: none; \
                             font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_MD,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        oninput: move |evt| {
                            let text = evt.value();
                            // Shared rather than moved: `oninput` is `FnMut`, so
                            // the setter must survive more than one keystroke.
                            let commit = std::rc::Rc::clone(&commit);
                            edit(draft, move |d| commit(d, text.clone()));
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
            footnote,
        }
    }
}

/// A selectable chip.
pub(super) fn chip_style(active: bool, posture: DialogPosture) -> String {
    format!(
        "{touch} box-sizing: border-box; padding: {py}px {px}px; border-radius: {r}px; \
         cursor: pointer; white-space: nowrap; font-size: {fs}px; \
         background: {bg}; border: 1px solid {border}; color: {fg};",
        touch = if posture.min_touch_px > 0.0 {
            format!("min-height: {}px;", posture.min_touch_px)
        } else {
            String::new()
        },
        py = tokens::SPACE_2,
        px = tokens::SPACE_3,
        r = tokens::RADIUS_MD,
        fs = tokens::FONT_SIZE_BODY,
        bg = if active {
            tokens::COLOR_SURFACE_3
        } else {
            tokens::COLOR_SURFACE_2
        },
        border = if active {
            tokens::COLOR_TAB_ACTIVE_INDICATOR
        } else {
            tokens::COLOR_BORDER_CHROME
        },
        fg = if active {
            tokens::COLOR_TEXT_ON_CHROME
        } else {
            tokens::COLOR_TEXT_ON_CHROME_SECONDARY
        },
    )
}
