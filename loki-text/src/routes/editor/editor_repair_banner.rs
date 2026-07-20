// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! "This document has problems that may stop it opening in Word — Repair?"
//! banner.
//!
//! Shown when a DOCX is opened whose OOXML child elements are out of the strict
//! schema order Microsoft Word enforces (Loki's tolerant reader opens it fine;
//! see `loki_ooxml::repair`). Offers a one-click lossless repair of the on-disk
//! file, or dismiss. Mirrors [`super::editor_font_warning::FontSubstitutionPanel`]:
//! amber `COLOR_CONTEXTUAL_TAB` accent (attention, not error), no `position:
//! fixed` / `box-shadow`, all strings via `fl!()`. Mounted at the panel boundary
//! (ADR-0013) so it owns its own hook scope.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_state::SaveStatus;

/// Detects Word-compatibility problems on open and returns the repair banner
/// element (empty when the document is clean or the banner was dismissed).
///
/// A self-contained custom hook: it owns the detection state + effect and the
/// repair action, so `EditorInner` adds a single line. Must be called
/// unconditionally at the top of the component (Dioxus hook-order rule).
///
/// `path_signal` is the route file token; `save_message` receives the
/// success/error status after a repair (surfaced by the existing save banner).
pub(super) fn use_repair_banner(
    path_signal: Signal<String>,
    save_message: Signal<Option<SaveStatus>>,
) -> Element {
    let mut report = use_signal(|| Option::<loki_ooxml::RepairReport>::None);
    let mut dismissed = use_signal(|| false);

    // Inspect the opened DOCX bytes off the render thread (never blocks the
    // open); resets for each newly-opened document.
    use_effect(move || {
        let p = path_signal();
        report.set(None);
        dismissed.set(false);
        spawn(async move {
            if let Some(r) = super::editor_load::analyze_open_docx(&p) {
                // Ignore a stale result if the user switched documents meanwhile.
                if path_signal.peek().as_str() == p && !r.is_clean() {
                    report.set(Some(r));
                }
            }
        });
    });

    let Some(r) = report() else {
        return rsx! {};
    };
    let count = r.findings.len();
    rsx! {
        RepairBanner {
            count,
            dismissed,
            on_repair: move |_| repair_now(path_signal, report, save_message),
        }
    }
}

/// Repairs the current file in place and reports the outcome via `save_message`.
fn repair_now(
    path_signal: Signal<String>,
    mut report: Signal<Option<loki_ooxml::RepairReport>>,
    mut save_message: Signal<Option<SaveStatus>>,
) {
    let path = path_signal.peek().clone();
    match super::editor_save::repair_document_file(&path) {
        Ok(n) => {
            report.set(None);
            save_message.set(Some(SaveStatus::ok(fl!(
                "editor-repair-done",
                count = n as i64
            ))));
        }
        Err(e) => {
            save_message.set(Some(SaveStatus::error(fl!(
                "editor-repair-error",
                reason = e.to_string()
            ))));
        }
    }
}

/// Inline repair banner. Renders nothing when there is nothing to repair
/// (`count == 0`) or the user dismissed it for this document.
///
/// **Touch targets:** the Repair and Dismiss buttons meet the 44×44 logical-px
/// minimum (WCAG 2.5.8) through their padding plus the banner row height,
/// matching the ribbon-button convention.
#[component]
pub(super) fn RepairBanner(
    count: usize,
    dismissed: Signal<bool>,
    on_repair: EventHandler<()>,
) -> Element {
    let mut dismissed = dismissed;
    if count == 0 || dismissed() {
        return rsx! {};
    }

    let container = format!(
        "display: flex; flex-direction: row; align-items: center; gap: {gap}px; \
         padding: {pv}px {ph}px; background: {bg}; border-top: 1px solid {border}; \
         border-bottom: 1px solid {border}; font-family: {ff}; color: {fg}; flex-shrink: 0;",
        gap = tokens::SPACE_2,
        pv = tokens::SPACE_2,
        ph = tokens::SPACE_4,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_CONTEXTUAL_TAB,
        ff = tokens::FONT_FAMILY_UI,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    );
    let repair_btn = format!(
        "padding: {pv}px {ph}px; background: {bg}; border: 1px solid {bg}; \
         border-radius: {r}px; color: {fg}; font-size: {size}px; font-weight: {w}; \
         cursor: pointer; flex-shrink: 0;",
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_3,
        bg = tokens::COLOR_ACCENT_PRIMARY,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
        w = tokens::FONT_WEIGHT_SEMIBOLD,
    );
    let dismiss_btn = format!(
        "padding: {pv}px {ph}px; background: {bg}; border: 1px solid {border}; \
         border-radius: {r}px; color: {fg}; font-size: {size}px; cursor: pointer; flex-shrink: 0;",
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_3,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    );

    rsx! {
        div { style: "{container}",
            span {
                style: format!("color: {}; font-weight: bold;", tokens::COLOR_CONTEXTUAL_TAB),
                "⚠ {fl!(\"editor-repair-title\")}"
            }
            span {
                style: format!(
                    "font-size: {}px; color: {};",
                    tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                {fl!("editor-repair-message", count = count as i64)}
            }
            div { style: "flex: 1;" }
            button {
                style: "{repair_btn}",
                onclick: move |_| on_repair.call(()),
                {fl!("editor-repair-action")}
            }
            button {
                style: "{dismiss_btn}",
                onclick: move |_| dismissed.set(true),
                {fl!("editor-repair-dismiss")}
            }
        }
    }
}
