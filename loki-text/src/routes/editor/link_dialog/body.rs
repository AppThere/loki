// SPDX-License-Identifier: Apache-2.0

//! The Insert link body: the kind picker, the per-kind address control, and the
//! shared display-text fields.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtField, AtSegmented, DialogPosture, at_control_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::LinkDraft;
use super::target::LinkKind;
use super::validate::AddressState;
use crate::editing::state::DocumentState;

/// Renders the dialog body for `kind`.
pub(super) fn form(
    doc_state: &Arc<Mutex<DocumentState>>,
    kind: LinkKind,
    mut kind_signal: Signal<LinkKind>,
    draft: Signal<Option<LinkDraft>>,
    posture: DialogPosture,
    state: &AddressState,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let compact = posture.full_screen;

    rsx! {
        div {
            style: format!(
                "flex: 1; min-width: 0; display: flex; flex-direction: column; \
                 gap: {gap}px; padding: {p}px; overflow-y: auto;",
                gap = tokens::SPACE_5,
                p = tokens::SPACE_5,
            ),

            // ── Target kind ───────────────────────────────────────────────────
            AtSegmented {
                options: LinkKind::ALL.iter().map(|k| k.label(compact)).collect::<Vec<_>>(),
                selected: kind.index(),
                min_touch_px: posture.min_touch_px,
                on_select: move |idx: usize| kind_signal.set(LinkKind::from_index(idx)),
            }

            // ── The address, in whichever form this kind takes ────────────────
            if kind == LinkKind::Document {
                { super::targets::target_picker(doc_state, draft, posture, &current) }
            } else {
                { address_field(kind, draft, posture, state, &current) }
            }

            // ── Shared fields ─────────────────────────────────────────────────
            { text_field(
                fl!("link-dialog-display-text"),
                current.display_text.clone(),
                fl!("link-dialog-display-text-placeholder"),
                draft,
                posture,
                |d, v| d.display_text = v,
                Some(fl!("link-dialog-display-from-selection")),
            ) }

            { text_field(
                fl!("link-dialog-description"),
                current.description.clone(),
                fl!("link-dialog-description-placeholder"),
                draft,
                posture,
                |d, v| d.description = v,
                None,
            ) }
        }
    }
}

/// The typed-address field, with its inline verdict underneath.
fn address_field(
    kind: LinkKind,
    mut draft: Signal<Option<LinkDraft>>,
    posture: DialogPosture,
    state: &AddressState,
    current: &LinkDraft,
) -> Element {
    let value = current.address(kind).to_string();
    let valid = state.can_insert();
    let message = state.message().map(str::to_string);
    let label = match kind {
        LinkKind::Email => fl!("link-dialog-address-email"),
        LinkKind::File => fl!("link-dialog-address-file"),
        _ => fl!("link-dialog-address-web"),
    };

    rsx! {
        AtField {
            label,
            control: rsx! {
                div {
                    style: format!(
                        "{base} border-color: {border};",
                        base = at_control_style(posture.min_touch_px, "width: 100%;"),
                        // The field itself carries the verdict, so the state is
                        // visible without reading the line beneath it.
                        border = if valid {
                            tokens::COLOR_TAB_ACTIVE_INDICATOR
                        } else {
                            tokens::COLOR_BORDER_CHROME
                        },
                    ),
                    input {
                        r#type: "text",
                        value: "{value}",
                        style: format!(
                            "flex: 1; min-width: 0; background: transparent; border: none; \
                             font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        oninput: move |evt| {
                            let mut next = draft.read().clone();
                            if let Some(d) = next.as_mut() {
                                d.set_address(kind, evt.value());
                            }
                            draft.set(next);
                        },
                    }
                }
            },
            footnote: match message {
                Some(text) => rsx! {
                    div {
                        style: format!(
                            "display: flex; align-items: center; gap: {gap}px; \
                             font-size: {fs}px; color: {fg};",
                            gap = tokens::SPACE_1,
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = if valid {
                                tokens::COLOR_TEXT_ACCENT
                            } else {
                                tokens::COLOR_CONTEXTUAL_TAB
                            },
                        ),
                        span { if valid { "\u{2713}" } else { "\u{203B}" } }
                        span { {text} }
                    }
                },
                None => rsx! {},
            },
        }
    }
}

/// A plain text field with an optional hint line beneath it.
fn text_field(
    label: String,
    value: String,
    placeholder: String,
    mut draft: Signal<Option<LinkDraft>>,
    posture: DialogPosture,
    set: impl Fn(&mut LinkDraft, String) + 'static,
    hint: Option<String>,
) -> Element {
    rsx! {
        AtField {
            label,
            control: rsx! {
                div {
                    style: at_control_style(posture.min_touch_px, "width: 100%;"),
                    input {
                        r#type: "text",
                        value: "{value}",
                        placeholder: "{placeholder}",
                        style: format!(
                            "flex: 1; min-width: 0; background: transparent; border: none; \
                             font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        oninput: move |evt| {
                            let mut next = draft.read().clone();
                            if let Some(d) = next.as_mut() {
                                set(d, evt.value());
                            }
                            draft.set(next);
                        },
                    }
                }
            },
            footnote: match hint {
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
