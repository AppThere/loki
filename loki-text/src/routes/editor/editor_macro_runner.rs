// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The in-app macro runner panel (Tools ▸ Macros — macro spec §9.3, Phase 5).
//!
//! Lists the runnable procedures in the (already-enabled) document's macro
//! modules and runs a chosen one against the live document via
//! [`super::editor_macro_run`]. Rendered in-flow above the ribbon like the
//! read-only viewer. All strings via `fl!()`; every control meets the 44×44
//! logical-pixel touch target (WCAG 2.5.8).
//!
//! v1 uses the capabilities the user has already granted in Document Security
//! (no mid-run prompts); see [`super::editor_macro_run`] for the posture.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;
use loki_macro_host::{Dialect, MacroRuntime, MacroService};

use super::editor_macro_notice::{MacroCtx, MacroView};
use super::editor_macro_run::{RunMessages, RunReport, run_macro};

/// One runnable procedure: which module it lives in and its name.
#[derive(Clone, PartialEq)]
struct ProcEntry {
    module: String,
    source: String,
    proc: String,
    dialect: Dialect,
}

/// The macro runner panel.
#[component]
pub(super) fn MacroRunnerPanel(
    ctx: MacroCtx,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    view: MacroView,
    dialect: Dialect,
    on_close: EventHandler<()>,
) -> Element {
    let svc = use_context::<MacroService>();
    let mut report = use_signal(|| None::<RunReport>);

    let procs = collect_procs(&view, dialect);

    let container = format!(
        "display: flex; flex-direction: column; gap: {gap}px; padding: {pv}px {ph}px; \
         background: {bg}; border-top: 1px solid {border}; border-bottom: 1px solid {border}; \
         font-family: {ff}; color: {fg}; flex-shrink: 0; max-height: 45vh; overflow-y: auto;",
        gap = tokens::SPACE_2,
        pv = tokens::SPACE_2,
        ph = tokens::SPACE_4,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        ff = tokens::FONT_FAMILY_UI,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    );

    rsx! {
        div { style: "{container}",
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                span {
                    style: format!("font-weight: bold; font-size: {}px;", tokens::FONT_SIZE_MD),
                    {fl!("macros-run-title")}
                }
                div { style: "flex: 1;" }
                button {
                    style: btn_style(false),
                    onclick: move |_| on_close.call(()),
                    {fl!("macros-run-close")}
                }
            }

            if procs.is_empty() {
                span {
                    style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    {fl!("macros-run-none")}
                }
            }

            for entry in procs {
                {
                    let ctx_run = ctx.clone();
                    let entry_run = entry.clone();
                    let svc_run = svc.clone();
                    rsx! {
                        div {
                            key: "{entry.module}:{entry.proc}",
                            style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                            span {
                                style: format!("flex: 1; font-size: {}px; font-family: {};", tokens::FONT_SIZE_LABEL, tokens::FONT_FAMILY_UI),
                                "{entry.module} · {entry.proc}"
                            }
                            button {
                                style: btn_style(true),
                                onclick: move |_| {
                                    let rep = do_run(&ctx_run, loro_doc, &svc_run, &entry_run);
                                    report.set(Some(rep));
                                },
                                {fl!("macros-run-action")}
                            }
                        }
                    }
                }
            }

            // Result of the most recent run.
            if let Some(rep) = report() {
                div {
                    style: format!(
                        "margin-top: 4px; padding: {p}px {p2}px; border-left: 3px solid {c}; \
                         font-size: {s}px; color: {fg}; display: flex; flex-direction: column; gap: 4px;",
                        p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                        c = if rep.ok { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_STATUS_ERROR_BORDER },
                        s = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME,
                    ),
                    span { "{rep.message}" }
                    for line in rep.dialog_log.iter() {
                        span {
                            style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_XS, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                            "🗩 {line}"
                        }
                    }
                }
            }
        }
    }
}

/// Collects the runnable procedures across all readable modules.
fn collect_procs(view: &MacroView, dialect: Dialect) -> Vec<ProcEntry> {
    let mut out = Vec::new();
    for module in &view.modules {
        if let Ok(names) = MacroRuntime::list_procedures(&module.source, dialect) {
            for proc in names {
                out.push(ProcEntry {
                    module: module.name.clone(),
                    source: module.source.clone(),
                    proc,
                    dialect,
                });
            }
        }
    }
    out
}

/// Runs `entry` against the live document, returning a report to display.
fn do_run(
    ctx: &MacroCtx,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    svc: &MacroService,
    entry: &ProcEntry,
) -> RunReport {
    let messages = run_messages();
    let doc_state = ctx.0.clone();
    let Some(payload) = super::editor_macro_notice::payload_of(&doc_state) else {
        return RunReport::failed(messages.unreadable);
    };
    let guard = loro_doc.read();
    let Some(loro) = guard.as_ref() else {
        return RunReport::failed(messages.unreadable);
    };
    run_macro(
        &doc_state,
        loro,
        svc,
        &payload,
        super::editor_macro_run::MacroCode {
            source: &entry.source,
            dialect: entry.dialect,
            proc: &entry.proc,
        },
        &messages,
    )
}

/// Resolved i18n strings for run outcomes.
fn run_messages() -> RunMessages {
    RunMessages {
        done: fl!("macros-run-done"),
        done_edited: fl!("macros-run-done-edited"),
        refused: fl!("macros-run-refused"),
        denied: fl!("macros-run-denied"),
        stopped: fl!("macros-run-stopped"),
        unreadable: fl!("macros-run-unreadable"),
    }
}

fn btn_style(accent: bool) -> String {
    let border = if accent {
        tokens::COLOR_MACRO_BADGE
    } else {
        tokens::COLOR_BORDER_CHROME
    };
    format!(
        "min-height: {th}px; padding: {pv}px {ph}px; background: {bg}; \
         border: 1px solid {border}; border-radius: {r}px; color: {fg}; \
         font-size: {size}px; cursor: pointer; flex-shrink: 0;",
        th = tokens::TOUCH_MIN,
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_3,
        border = border,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    )
}
