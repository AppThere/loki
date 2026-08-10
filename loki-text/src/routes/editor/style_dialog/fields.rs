// SPDX-License-Identifier: Apache-2.0

//! Field builders shared by the paragraph dialog's tab bodies.
//!
//! Each builder pairs a control with the provenance line beneath it, so a tab
//! body cannot render a control and forget where its value came from — the one
//! thing every field in this dialog must say (design note 01).

use appthere_ui::tokens;
use appthere_ui::{AtField, AtProvenanceLine, DialogPosture, at_control_style};
use dioxus::prelude::*;
use loki_doc_model::style::{StyleCatalog, StyleId};

use super::draft::ParaDialogDraft;
use super::rows::{RowSource, resolve_row};

/// The draft signal every field writes through.
pub(super) type DraftSignal = Signal<Option<ParaDialogDraft>>;

/// The signal that re-targets the dialog, for the "Edit there" jump.
pub(super) type OpenSignal = Signal<Option<String>>;

/// Renders the provenance line for one property.
///
/// `on_reset` clears the local override. The jump is wired to `open_style`, so
/// following it re-targets the whole dialog at the ancestor that owns the value
/// — the `ancestor_id` the resolver already carries (design note 02).
pub(super) fn provenance_line(
    source: &RowSource,
    value: Option<&str>,
    posture: DialogPosture,
    mut open_style: OpenSignal,
    mut draft: DraftSignal,
    reset: impl Fn(&mut ParaDialogDraft) + 'static,
) -> Element {
    let ancestor = source.ancestor.clone();
    let jump_label = source.jump_label();
    let reset_label = source.reset_label();
    let text = source.text(value);
    let kind = source.kind;

    rsx! {
        AtProvenanceLine {
            kind,
            text,
            min_touch_px: posture.min_touch_px,
            jump_label,
            on_jump: ancestor.map(|id| EventHandler::new(move |()| {
                open_style.set(Some(id.as_str().to_string()));
            })),
            reset_label,
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

/// A numeric field: caption, text input, and the provenance line.
///
/// `buffer` reads the draft's text buffer for this control and `commit` writes
/// both the buffer and — when it parses — the model property. Splitting them is
/// what lets a half-typed `1.` survive a redraw without clearing the property.
#[allow(clippy::too_many_arguments)]
pub(super) fn numeric_field<T: Clone>(
    label: String,
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
    get: impl Fn(&loki_doc_model::style::ParagraphStyle) -> Option<T> + 'static,
    fmt: impl Fn(&T) -> String + 'static,
    buffer: impl Fn(&ParaDialogDraft) -> String,
    commit: impl Fn(&mut ParaDialogDraft, String) + 'static,
    reset: impl Fn(&mut ParaDialogDraft) + 'static,
    suffix: Option<String>,
    extra_style: String,
) -> Element {
    let value = draft.read().as_ref().map(&buffer).unwrap_or_default();
    let row = resolve_row(catalog, id, get, fmt);
    let footnote = row.as_ref().map(|(source, resolved)| {
        provenance_line(
            source,
            resolved.as_deref(),
            posture,
            open_style,
            draft,
            reset,
        )
    });

    rsx! {
        AtField {
            label,
            extra_style,
            control: rsx! {
                div {
                    style: at_control_style(posture.min_touch_px, "width: 100%;"),
                    input {
                        r#type: "text",
                        value: "{value}",
                        style: format!(
                            "flex: 1; min-width: 0; background: transparent; \
                             border: none; font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_MD,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        oninput: move |evt| {
                            let mut draft = draft;
                            let mut next = draft.read().clone();
                            if let Some(d) = next.as_mut() {
                                commit(d, evt.value());
                            }
                            draft.set(next);
                        },
                    }
                    if let Some(unit) = suffix {
                        span {
                            style: format!(
                                "flex-shrink: 0; font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_LABEL,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            {unit}
                        }
                    }
                }
            },
            footnote: footnote.unwrap_or_else(|| rsx! {}),
        }
    }
}

/// A plain text field with no provenance line — the identity fields on the
/// General tab, which are not resolved properties.
pub(super) fn text_field(
    label: String,
    value: String,
    draft: DraftSignal,
    posture: DialogPosture,
    commit: impl Fn(&mut ParaDialogDraft, String) + 'static,
    extra_style: String,
) -> Element {
    rsx! {
        AtField {
            label,
            extra_style,
            control: rsx! {
                div {
                    style: at_control_style(posture.min_touch_px, "width: 100%;"),
                    input {
                        r#type: "text",
                        value: "{value}",
                        style: format!(
                            "flex: 1; min-width: 0; background: transparent; \
                             border: none; font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_MD,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        oninput: move |evt| {
                            let mut draft = draft;
                            let mut next = draft.read().clone();
                            if let Some(d) = next.as_mut() {
                                commit(d, evt.value());
                            }
                            draft.set(next);
                        },
                    }
                }
            },
        }
    }
}

/// The grid a tab body lays its fields out on: two columns at pointer density,
/// one when the dialog is a Compact sheet.
///
/// COMPAT(dioxus-native): `display: grid` with `grid-template-columns` is used
/// throughout the existing editor chrome and is confirmed working in this Blitz
/// build; `grid-column` spans are used only for full-width rows.
#[must_use]
pub(super) fn body_grid_style(posture: DialogPosture) -> String {
    format!(
        "flex: 1; min-width: 0; display: grid; \
         grid-template-columns: {cols}; gap: {gy}px {gx}px; \
         align-content: start; padding: {py}px {px}px; overflow-y: auto;",
        cols = if posture.full_screen {
            "1fr"
        } else {
            "1fr 1fr"
        },
        gy = tokens::SPACE_5,
        gx = tokens::SPACE_5,
        py = tokens::SPACE_5,
        px = tokens::SPACE_5,
    )
}

/// A field that should span the whole body grid.
#[must_use]
pub(super) fn full_width(posture: DialogPosture) -> String {
    if posture.full_screen {
        String::new()
    } else {
        "grid-column: 1 / -1;".to_string()
    }
}
