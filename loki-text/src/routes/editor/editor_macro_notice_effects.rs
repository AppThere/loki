// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The `MacroNoticeBar` background effects, split out of `editor_macro_notice`
//! for the 300-line ceiling. Both are hooks: call them unconditionally, once, in
//! stable order from the component body.

use dioxus::prelude::*;
use loki_macro_host::MacroService;

use super::editor_macro_notice::{MacroCtx, MacroView, payload_of};

/// Fires on-open handlers when a newly-opened document authorizes auto-run
/// (spec §5.6). Reads `loro_doc` so it re-runs on document load; guarded per
/// payload-hash so it fires at most once per document. Gated by
/// `authorize_auto_run` (trusted + `auto_run_open`); the runner re-checks the
/// token, so nothing fires without the flag.
pub(super) fn use_auto_run_effect(
    ctx: MacroCtx,
    svc: MacroService,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut runner: Signal<Option<MacroView>>,
    mut runner_auto: Signal<bool>,
) {
    let mut fired = use_signal(|| None::<[u8; 32]>);
    use_effect(move || {
        let _loaded = loro_doc.read().is_some();
        if let Some(payload) = payload_of(&ctx.0) {
            let key = payload.payload_hash();
            if fired() != Some(key) && svc.authorize_auto_run(&payload).is_some() {
                let v = super::editor_macro_extract::extract_view(&payload);
                if !v.modules.is_empty() {
                    fired.set(Some(key));
                    runner.set(Some(v));
                    runner_auto.set(true);
                }
            }
        }
    });
}

/// Dispatches a MACROBUTTON click (spec §6): when the document's macros are
/// enabled, opens the runner on the named procedure through the gated path; when
/// disabled, prompts to enable first — a click never runs a macro from an
/// untrusted document.
#[allow(clippy::too_many_arguments)]
pub(super) fn use_click_dispatch_effect(
    ctx: MacroCtx,
    svc: MacroService,
    mut macro_run_request: Signal<Option<String>>,
    mut runner: Signal<Option<MacroView>>,
    mut runner_auto: Signal<bool>,
    mut run_proc: Signal<Option<String>>,
    mut trust_open: Signal<bool>,
) {
    use_effect(move || {
        if let Some(name) = macro_run_request() {
            if let Some(payload) = payload_of(&ctx.0) {
                if svc.decision_for(&payload).is_enabled() {
                    let v = super::editor_macro_extract::extract_view(&payload);
                    if !v.modules.is_empty() {
                        runner_auto.set(false);
                        run_proc.set(Some(name));
                        runner.set(Some(v));
                    }
                } else {
                    trust_open.set(true);
                }
            }
            macro_run_request.set(None);
        }
    });
}
