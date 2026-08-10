// SPDX-License-Identifier: Apache-2.0

//! The metadata dialog's tab bodies.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, DialogPosture, at_control_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_metadata::{MetaDraft, MetaField};
use super::tabs::{MetaTab, is_epub_required};
use crate::editing::state::DocumentState;

/// Renders the active tab's body.
pub(super) fn tab_body(
    tab: MetaTab,
    doc_state: &Arc<Mutex<DocumentState>>,
    draft: Signal<Option<MetaDraft>>,
    posture: DialogPosture,
    missing: usize,
) -> Element {
    match tab {
        MetaTab::Accessibility => super::tab_review::accessibility(doc_state, posture),
        MetaTab::Statistics => super::tab_review::statistics(doc_state, posture),
        _ => fields(tab, draft, posture, missing),
    }
}

/// The grid the field tabs lay out on.
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
pub(super) fn span_all(posture: DialogPosture) -> String {
    if posture.full_screen {
        String::new()
    } else {
        "grid-column: 1 / -1;".to_string()
    }
}

/// An editable-field tab.
fn fields(
    tab: MetaTab,
    mut draft: Signal<Option<MetaDraft>>,
    posture: DialogPosture,
    missing: usize,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };

    rsx! {
        div {
            style: grid_style(posture),

            // Note 16: the running count of empty-but-required fields, stated
            // where the fields are rather than only on the tab strip.
            if tab == MetaTab::General && missing > 0 {
                AtDialogNotice {
                    tone: AtNoticeTone::Caution,
                    extra_style: span_all(posture),
                    message: rsx! {
                        { fl!("meta-dialog-missing-count", count = missing as i64) }
                    },
                }
            }

            for field in tab.fields().iter().copied() {
                {
                    let value = current
                        .values
                        .iter()
                        .find(|(f, _)| *f == field)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default();
                    let empty_required = is_epub_required(field) && value.trim().is_empty();
                    let wide = MetaTab::is_wide_field(field);
                    rsx! {
                        AtField {
                            key: "{field.label()}",
                            label: field.label(),
                            extra_style: if wide { span_all(posture) } else { String::new() },
                            control: rsx! {
                                div {
                                    style: format!(
                                        "{base} border-style: {style}; border-color: {color};",
                                        base = at_control_style(posture.min_touch_px, "width: 100%;"),
                                        // Note 16: a required field left empty is
                                        // dashed rather than hidden, so it reads as
                                        // "not filled in" rather than "not needed".
                                        style = if empty_required { "dashed" } else { "solid" },
                                        color = if empty_required {
                                            tokens::COLOR_CONTEXTUAL_TAB
                                        } else {
                                            tokens::COLOR_BORDER_CHROME
                                        },
                                    ),
                                    input {
                                        r#type: "text",
                                        value: "{value}",
                                        placeholder: field_placeholder(field),
                                        style: format!(
                                            "flex: 1; min-width: 0; background: transparent; \
                                             border: none; font-size: {fs}px; color: {fg};",
                                            fs = tokens::FONT_SIZE_BODY,
                                            fg = tokens::COLOR_TEXT_ON_CHROME,
                                        ),
                                        oninput: move |evt| {
                                            let mut next = draft.read().clone();
                                            if let Some(d) = next.as_mut()
                                                && let Some(slot) =
                                                    d.values.iter_mut().find(|(f, _)| *f == field)
                                            {
                                                slot.1 = evt.value();
                                            }
                                            draft.set(next);
                                        },
                                    }
                                    if empty_required {
                                        span {
                                            style: format!(
                                                "flex-shrink: 0; font-size: {fs}px; color: {fg};",
                                                fs = tokens::FONT_SIZE_XS,
                                                fg = tokens::COLOR_CONTEXTUAL_TAB,
                                            ),
                                            { fl!("meta-dialog-required-badge") }
                                        }
                                    }
                                }
                            },
                            footnote: match field_hint(field) {
                                Some(hint) => rsx! {
                                    div {
                                        style: format!(
                                            "display: flex; align-items: center; gap: {gap}px; \
                                             font-size: {fs}px; color: {fg};",
                                            gap = tokens::SPACE_1,
                                            fs = tokens::FONT_SIZE_LABEL,
                                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                        ),
                                        span { "\u{21B3}" }
                                        span { {hint} }
                                    }
                                },
                                None => rsx! {},
                            },
                        }
                    }
                }
            }

            if tab == MetaTab::Identifiers {
                AtDialogNotice {
                    tone: AtNoticeTone::Caution,
                    extra_style: span_all(posture),
                    message: rsx! { { fl!("meta-dialog-identifier-warning") } },
                }
            }
        }
    }
}

/// The hint under a field whose storage format is not obvious from its label.
fn field_hint(field: MetaField) -> Option<String> {
    match field {
        // Note 18: both are semicolon-joined strings in the model. The UI could
        // show chips, but the separator has to be stated somewhere or a name
        // containing a comma round-trips as two people.
        MetaField::Contributors => Some(fl!("meta-dialog-hint-contributors")),
        MetaField::Keywords => Some(fl!("meta-dialog-hint-keywords")),
        MetaField::Language => Some(fl!("meta-dialog-hint-language")),
        MetaField::Issued => Some(fl!("meta-dialog-hint-issued")),
        MetaField::Identifier => Some(fl!("meta-dialog-hint-identifier")),
        MetaField::DcType => Some(fl!("meta-dialog-hint-dc-type")),
        _ => None,
    }
}

/// The placeholder shown in an empty field.
fn field_placeholder(field: MetaField) -> String {
    match field {
        MetaField::Language => fl!("meta-dialog-placeholder-language"),
        MetaField::Issued => fl!("meta-dialog-placeholder-issued"),
        _ => fl!("meta-dialog-placeholder-empty"),
    }
}

/// The footer's revision line.
pub(super) fn revision_line(doc_state: &Arc<Mutex<DocumentState>>) -> String {
    let revision = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().and_then(|d| d.meta.revision))
        .unwrap_or(0);
    fl!("meta-dialog-revision", revision = i64::from(revision))
}

/// Groups a count with thin spaces, so `118203` reads as `118 203` rather than
/// being counted digit by digit.
pub(super) fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202F}');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
