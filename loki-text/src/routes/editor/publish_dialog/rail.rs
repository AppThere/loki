// SPDX-License-Identifier: Apache-2.0

//! The standing preflight surface: a docked rail at Expanded, a collapsed
//! summary card below it, and the row shared by both.
//!
//! Never a modal that appears after Publish is pressed (design note 27) — which
//! is why this is a body-level surface rather than a second dialog, and why the
//! collapsed card still shows every warning and error, hiding only the passes.

use appthere_ui::{AtDialogButton, DialogPosture, at_field_label_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::preflight::{Finding, FixIn, Preflight, Severity};

/// The docked preflight rail.
pub(super) fn rail(
    report: &Preflight,
    posture: DialogPosture,
    open_metadata: Signal<bool>,
    open: Signal<bool>,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: column; height: 100%;",
            div {
                style: format!(
                    "flex-shrink: 0; padding: {py}px {px}px; \
                     border-bottom: 1px solid {border}; {label}",
                    py = tokens::SPACE_3,
                    px = tokens::SPACE_4,
                    border = tokens::COLOR_BORDER_CHROME,
                    label = at_field_label_style(),
                ),
                { fl!("publish-dialog-preflight") }
            }
            div {
                style: format!(
                    "flex: 1; display: flex; flex-direction: column; gap: {gap}px; padding: {p}px;",
                    gap = tokens::SPACE_3,
                    p = tokens::SPACE_4,
                ),
                for (i, finding) in report.findings.iter().enumerate() {
                    { finding_row(i, finding, posture, open_metadata, open) }
                }
            }
            div {
                style: format!(
                    "margin-top: auto; flex-shrink: 0; padding: {py}px {px}px; \
                     border-top: 1px solid {border}; font-size: {fs}px; color: {fg};",
                    py = tokens::SPACE_3,
                    px = tokens::SPACE_4,
                    border = tokens::COLOR_BORDER_CHROME,
                    fs = tokens::FONT_SIZE_META,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { report.summary() }
            }
        }
    }
}

/// The collapsed preflight card shown below Expanded.
pub(super) fn summary_card(
    report: &Preflight,
    posture: DialogPosture,
    mut preflight_open: Signal<bool>,
    open_metadata: Signal<bool>,
    open: Signal<bool>,
) -> Element {
    let expanded = *preflight_open.read();
    // Collapsed, the card still shows what is wrong — only the passes are
    // hidden. A summary that hid the warnings too would be a modal by another
    // name.
    let notable: Vec<(usize, &Finding)> = report
        .findings
        .iter()
        .enumerate()
        .filter(|(_, f)| expanded || f.severity != Severity::Pass)
        .collect();

    rsx! {
        div {
            style: format!(
                "flex-shrink: 0; margin: {m}px {m}px 0; padding: {p}px; \
                 background: {bg}; border: 1px solid {border}; border-radius: {r}px; \
                 display: flex; flex-direction: column; gap: {gap}px;",
                m = tokens::SPACE_5,
                p = tokens::SPACE_3,
                bg = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_BORDER_CHROME,
                r = tokens::RADIUS_MD,
                gap = tokens::SPACE_3,
            ),
            button {
                aria_expanded: if expanded { "true" } else { "false" },
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     justify-content: space-between; background: transparent; \
                     border: none; cursor: pointer; padding: 0; {touch}",
                    touch = if posture.min_touch_px > 0.0 {
                        format!("min-height: {}px;", posture.min_touch_px)
                    } else {
                        String::new()
                    },
                ),
                onclick: move |evt| {
                    evt.stop_propagation();
                    let now = expanded;
                    preflight_open.set(!now);
                },
                span { style: at_field_label_style(), { fl!("publish-dialog-preflight") } }
                span {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { report.summary() }
                }
            }
            for (i, finding) in notable {
                { finding_row(i, finding, posture, open_metadata, open) }
            }
        }
    }
}

/// One preflight row, with its jump when it has somewhere to go.
fn finding_row(
    index: usize,
    finding: &Finding,
    posture: DialogPosture,
    mut open_metadata: Signal<bool>,
    mut open: Signal<bool>,
) -> Element {
    let (marker, colour) = match finding.severity {
        Severity::Pass => ("\u{2713}", tokens::COLOR_TEXT_ACCENT),
        Severity::Warning => ("!", tokens::COLOR_CONTEXTUAL_TAB),
        Severity::Error => ("\u{2715}", tokens::COLOR_STATUS_ERROR_BORDER),
    };
    let can_jump = finding.fix_in == FixIn::Metadata;

    rsx! {
        div {
            key: "finding-{index}",
            style: format!(
                "display: flex; flex-direction: row; align-items: flex-start; \
                 gap: {gap}px; font-size: {fs}px; line-height: 1.5; color: {fg};",
                gap = tokens::SPACE_2,
                fs = tokens::FONT_SIZE_META,
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            span { style: format!("flex-shrink: 0; color: {colour};"), {marker} }
            div {
                style: "display: flex; flex-direction: column;",
                span { {finding.message.clone()} }
                // Note 28: each warning links to the dialog that fixes it.
                if can_jump {
                    AtDialogButton {
                        label: fl!("publish-dialog-open-metadata"),
                        tertiary: true,
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| {
                            open.set(false);
                            open_metadata.set(true);
                        },
                    }
                }
            }
        }
    }
}
