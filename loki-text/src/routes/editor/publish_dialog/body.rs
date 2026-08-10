// SPDX-License-Identifier: Apache-2.0

//! The publish dialog's tab bodies and the preflight rail.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::preflight::Preflight;
use super::tabs::{PublishOptions, PublishTab, TocDepth};
use crate::editing::state::DocumentState;

/// The dialog's subtitle: format, section count, and the estimate.
#[must_use]
pub(super) fn subtitle(doc_state: &Arc<Mutex<DocumentState>>) -> String {
    let sections = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| d.sections.len()))
        .unwrap_or(0);
    fl!("publish-dialog-subtitle", sections = sections as i64)
}

/// Renders the active tab, with the preflight docked beside it at Expanded.
#[allow(clippy::too_many_arguments)]
pub(super) fn tab_body(
    tab: PublishTab,
    doc_state: &Arc<Mutex<DocumentState>>,
    options: Signal<PublishOptions>,
    posture: DialogPosture,
    report: &Preflight,
    preflight_open: Signal<bool>,
    open_metadata: Signal<bool>,
    open: Signal<bool>,
) -> Element {
    let form = match tab {
        PublishTab::Content => content(options, posture),
        PublishTab::Metadata => metadata(doc_state, posture),
        PublishTab::Accessibility => accessibility(doc_state, posture),
        PublishTab::Fonts => super::tab_fonts::fonts_tab(doc_state, posture),
        PublishTab::Output => super::tab_fonts::output(doc_state, posture),
    };

    rsx! {
        div {
            style: "flex: 1; min-width: 0; display: flex; flex-direction: row;",
            div {
                style: "flex: 1; min-width: 0; display: flex; flex-direction: column; overflow-y: auto;",
                // Below Expanded the rail becomes a collapsed summary card at
                // the top of the body — never a modal after Publish (note 27).
                if !posture.preview_docked {
                    { super::rail::summary_card(report, posture, preflight_open, open_metadata, open) }
                }
                {form}
            }
            if posture.preview_docked {
                div {
                    style: format!(
                        "width: {w}px; flex-shrink: 0; border-left: 1px solid {border}; \
                         display: flex; flex-direction: column; overflow-y: auto;",
                        w = tokens::DIALOG_PREVIEW_RAIL_PX,
                        border = tokens::COLOR_BORDER_CHROME,
                    ),
                    { super::rail::rail(report, posture, open_metadata, open) }
                }
            }
        }
    }
}

/// The grid the option tabs lay out on.
pub(super) fn grid(posture: DialogPosture) -> String {
    format!(
        "display: grid; grid-template-columns: {cols}; gap: {gy}px {gx}px; \
         align-content: start; padding: {p}px;",
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

/// The **Content** tab.
fn content(mut options: Signal<PublishOptions>, posture: DialogPosture) -> Element {
    let current = options.read().clone();

    rsx! {
        div {
            style: grid(posture),

            AtField {
                label: fl!("publish-dialog-toc-depth"),
                extra_style: span_all(posture),
                control: rsx! {
                    AtSegmented {
                        options: TocDepth::CHOICES
                            .iter()
                            .map(|d| TocDepth::label(*d))
                            .collect::<Vec<_>>(),
                        selected: current.toc_depth.index(),
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            let mut next = options.read().clone();
                            next.toc_depth = TocDepth::from_index(idx);
                            options.set(next);
                        },
                    }
                },
            }

            // Chapter splitting, the cover, NCX and page-list markers have no
            // representation in `EpubOptions` — the exporter emits one content
            // document, always generates a navigation document, and packages no
            // cover. They are named rather than drawn as controls that would be
            // collected and then dropped.
            // TODO(epub-options): extend `loki_epub::EpubOptions` with
            // `split_at_heading`, `cover`, `ncx` and `page_list`, honour them in
            // `EpubExport`, then surface them here.
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("publish-dialog-content-unsupported") } },
            }
        }
    }
}

/// The **Metadata** tab — a read-through of the package document.
fn metadata(doc_state: &Arc<Mutex<DocumentState>>, posture: DialogPosture) -> Element {
    let guard = doc_state.lock().ok();
    let meta = guard
        .as_ref()
        .and_then(|s| s.document.as_ref().map(|d| d.meta.clone()))
        .unwrap_or_default();
    let dc = &meta.dublin_core;
    let rows: Vec<(String, Option<String>)> = vec![
        ("dc:title".to_string(), meta.title.clone()),
        ("dc:creator".to_string(), meta.creator.clone()),
        (
            "dc:language".to_string(),
            meta.language.as_ref().map(|l| l.as_str().to_string()),
        ),
        ("dc:identifier".to_string(), dc.identifier.clone()),
        ("dc:publisher".to_string(), dc.publisher.clone()),
        ("dc:rights".to_string(), dc.rights.clone()),
    ];

    rsx! {
        div {
            style: grid(posture),

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                // A confirmation surface, not a second copy: every row here is
                // the same store the metadata dialog edits.
                message: rsx! { { fl!("publish-dialog-metadata-note") } },
            }

            div {
                style: format!(
                    "{span} border: 1px solid {border}; border-radius: {r}px; overflow: hidden;",
                    span = span_all(posture),
                    border = tokens::COLOR_BORDER_CHROME,
                    r = tokens::RADIUS_MD,
                ),
                for (key, value) in rows.iter() {
                    div {
                        key: "{key}",
                        style: format!(
                            "display: flex; flex-direction: row; align-items: center; \
                             gap: {gap}px; padding: {py}px {px}px; \
                             border-bottom: 1px solid {border}; font-size: {fs}px;",
                            gap = tokens::SPACE_3,
                            py = tokens::SPACE_2,
                            px = tokens::SPACE_3,
                            border = tokens::COLOR_BORDER_CHROME,
                            fs = tokens::FONT_SIZE_BODY,
                        ),
                        span {
                            style: format!(
                                "width: 140px; flex-shrink: 0; font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_META,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            {key.clone()}
                        }
                        span {
                            style: format!(
                                "flex: 1; min-width: 0; color: {fg};",
                                fg = if value.is_some() {
                                    tokens::COLOR_TEXT_ON_CHROME
                                } else {
                                    tokens::COLOR_TEXT_ON_CHROME_SECONDARY
                                },
                            ),
                            {
                                value
                                    .clone()
                                    .unwrap_or_else(|| fl!("publish-dialog-not-set"))
                            }
                        }
                        span {
                            style: format!(
                                "flex-shrink: 0; font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_META,
                                fg = if value.is_some() {
                                    tokens::COLOR_TEXT_ACCENT
                                } else {
                                    tokens::COLOR_CONTEXTUAL_TAB
                                },
                            ),
                            if value.is_some() { "\u{2713}" } else { "!" }
                        }
                    }
                }
            }
        }
    }
}

/// The **Accessibility** tab.
fn accessibility(doc_state: &Arc<Mutex<DocumentState>>, posture: DialogPosture) -> Element {
    let report = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| super::preflight::run(d)))
        .unwrap_or_default();
    let clean = report.warnings() == 0 && report.errors() == 0;

    rsx! {
        div {
            style: grid(posture),
            AtDialogNotice {
                tone: if clean { AtNoticeTone::Positive } else { AtNoticeTone::Caution },
                extra_style: span_all(posture),
                message: rsx! {
                    if clean {
                        { fl!("publish-dialog-a11y-ok") }
                    } else {
                        { fl!("publish-dialog-a11y-issues", count = report.warnings() as i64) }
                    }
                },
            }
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                // The claims themselves have no model — the same gap the
                // metadata dialog's Accessibility tab reports.
                message: rsx! { { fl!("publish-dialog-a11y-unsupported") } },
            }
        }
    }
}
