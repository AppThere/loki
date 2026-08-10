// SPDX-License-Identifier: Apache-2.0

//! The metadata dialog's two read-only tabs: **Accessibility** and
//! **Statistics**.
//!
//! Both are computed from the live document rather than stored, so every row
//! here is a display, not a field — which is why they share one read-only row
//! renderer instead of the editable one next door.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, DialogPosture, at_control_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::body::{grid_style, span_all, thousands};
use super::stats::collect;
use crate::editing::state::DocumentState;

/// The Accessibility tab.
///
/// # Derived and read-only, because the model has nowhere to put it
///
/// EPUB accessibility metadata (`schema:accessMode`, `accessibilityFeature`,
/// `accessibilitySummary`, `accessibilityHazard`, `dcterms:conformsTo`) has no
/// representation in `DocumentMeta` at all. Rather than draw six inputs that
/// discard what is typed, the tab reports what Loki **can** derive from the
/// document — which is most of what a conformant package needs — and says which
/// claims cannot yet be edited.
pub(super) fn accessibility(
    doc_state: &Arc<Mutex<DocumentState>>,
    posture: DialogPosture,
) -> Element {
    let stats = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| collect(d)))
        .unwrap_or_default();
    // "textual" alone is only true when there is nothing else to perceive.
    let textual_only = stats.images == 0;

    rsx! {
        div {
            style: grid_style(posture),

            AtDialogNotice {
                tone: if textual_only { AtNoticeTone::Positive } else { AtNoticeTone::Caution },
                extra_style: span_all(posture),
                message: rsx! {
                    if textual_only {
                        { fl!("meta-dialog-a11y-textual") }
                    } else {
                        { fl!("meta-dialog-a11y-visual", images = stats.images as i64) }
                    }
                },
            }

            { readonly_field(
                fl!("meta-dialog-a11y-access-mode"),
                if textual_only {
                    fl!("meta-dialog-a11y-mode-textual")
                } else {
                    fl!("meta-dialog-a11y-mode-textual-visual")
                },
                Some(fl!(
                    "meta-dialog-a11y-derived",
                    images = stats.images as i64,
                    tables = stats.tables as i64
                )),
                posture,
                String::new(),
            ) }

            { readonly_field(
                fl!("meta-dialog-a11y-features"),
                fl!("meta-dialog-a11y-features-value"),
                Some(fl!("meta-dialog-a11y-features-note")),
                posture,
                String::new(),
            ) }

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                // TODO(meta-accessibility): add `accessibility` to
                // `DocumentMeta` (access modes, features, hazards, summary and
                // conformsTo), round-trip it through the Loro bridge, and write
                // it from `loki-epub`'s package builder — then these become
                // editable fields rather than derived claims.
                message: rsx! { { fl!("meta-dialog-a11y-unsupported") } },
            }
        }
    }
}

/// The Statistics tab — derived counts, plus the dates the model does keep.
pub(super) fn statistics(doc_state: &Arc<Mutex<DocumentState>>, posture: DialogPosture) -> Element {
    let guard = doc_state.lock().ok();
    let document = guard.as_ref().and_then(|s| s.document.as_ref());
    let stats = document.map(|d| collect(d)).unwrap_or_default();
    let meta = document.map(|d| d.meta.clone()).unwrap_or_default();
    let cards: [(String, String); 6] = [
        (fl!("meta-dialog-stat-words"), thousands(stats.words)),
        (
            fl!("meta-dialog-stat-characters"),
            thousands(stats.characters),
        ),
        (
            fl!("meta-dialog-stat-paragraphs"),
            thousands(stats.paragraphs),
        ),
        (fl!("meta-dialog-stat-tables"), thousands(stats.tables)),
        (fl!("meta-dialog-stat-images"), thousands(stats.images)),
        (fl!("meta-dialog-stat-notes"), thousands(stats.notes)),
    ];

    rsx! {
        div {
            style: grid_style(posture),

            div {
                style: format!(
                    "{span} display: grid; grid-template-columns: repeat({n}, 1fr); gap: {gap}px;",
                    span = span_all(posture),
                    n = if posture.full_screen { 2 } else { 3 },
                    gap = tokens::SPACE_3,
                ),
                for (label, value) in cards.iter() {
                    div {
                        key: "{label}",
                        style: format!(
                            "background: {bg}; border: 1px solid {border}; \
                             border-radius: {r}px; padding: {p}px;",
                            bg = tokens::COLOR_SURFACE_2,
                            border = tokens::COLOR_BORDER_CHROME,
                            r = tokens::RADIUS_MD,
                            p = tokens::SPACE_3,
                        ),
                        div {
                            style: format!(
                                "margin-bottom: {mb}px; font-size: {fs}px; color: {fg};",
                                mb = tokens::SPACE_1,
                                fs = tokens::FONT_SIZE_LABEL,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            {label.clone()}
                        }
                        div {
                            style: format!(
                                "font-size: {fs}px; font-weight: {fw}; color: {fg};",
                                fs = tokens::FONT_SIZE_HEADING,
                                fw = tokens::FONT_WEIGHT_SEMIBOLD,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            {value.clone()}
                        }
                    }
                }
            }

            { readonly_field(
                fl!("meta-dialog-stat-created"),
                meta.created
                    .map(|d| d.format("%e %b %Y").to_string())
                    .unwrap_or_else(|| fl!("meta-dialog-stat-unknown")),
                None,
                posture,
                String::new(),
            ) }
            { readonly_field(
                fl!("meta-dialog-stat-modified"),
                meta.modified
                    .map(|d| d.format("%e %b %Y").to_string())
                    .unwrap_or_else(|| fl!("meta-dialog-stat-unknown")),
                None,
                posture,
                String::new(),
            ) }

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("meta-dialog-stat-note") } },
            }
        }
    }
}

/// A read-only value, styled as a field so the tabs read consistently.
fn readonly_field(
    label: String,
    value: String,
    hint: Option<String>,
    posture: DialogPosture,
    extra_style: String,
) -> Element {
    rsx! {
        AtField {
            label,
            extra_style,
            control: rsx! {
                div {
                    style: format!(
                        "{base} color: {fg};",
                        base = at_control_style(posture.min_touch_px, "width: 100%;"),
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    span { style: "flex: 1; min-width: 0;", {value} }
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
