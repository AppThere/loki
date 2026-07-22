// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The `MacroNoticeBar` background effects, split out of `editor_macro_notice`
//! for the 300-line ceiling. Both are hooks: call them unconditionally, once, in
//! stable order from the component body.

use dioxus::prelude::*;
use loki_macro_host::MacroService;

use super::editor_macro_notice::{MacroCtx, MacroView, payload_of};

/// The three signals that together launch the macro runner, bundled so the
/// effects below stay well under the argument-count lint (all `Signal`s are
/// `Copy`, so this is a cheap handle).
#[derive(Clone, Copy)]
pub(super) struct RunnerLaunch {
    /// The extracted view to run; `Some` mounts the runner panel.
    pub(super) view: Signal<Option<MacroView>>,
    /// Whether the run is an auto-run-on-open (vs. an explicit invocation).
    pub(super) auto: Signal<bool>,
    /// A specific procedure to run (a MACROBUTTON click), else the picker.
    pub(super) proc: Signal<Option<String>>,
}

/// Verifies the document's preserved macro signature once per payload as it
/// loads (ADR-0014 §4.5, 8A.8) and records the verdict in the [`MacroService`],
/// so the Document Security panel and the enable-at-open gate reflect a
/// trusted-publisher signature. Reads `loro_doc` so it re-runs on document load;
/// guarded per payload-hash so it verifies at most once per document.
pub(super) fn use_verify_signature_effect(
    ctx: MacroCtx,
    svc: MacroService,
    loro_doc: Signal<Option<loro::LoroDoc>>,
) {
    let mut verified = use_signal(|| None::<[u8; 32]>);
    use_effect(move || {
        let _loaded = loro_doc.read().is_some();
        if let Some(payload) = payload_of(&ctx.0) {
            let key = payload.payload_hash();
            if verified() != Some(key) {
                verified.set(Some(key));
                svc.verify_and_record(&payload);
            }
        }
    });
}

/// Fires on-open handlers when a newly-opened document authorizes auto-run
/// (spec §5.6). Reads `loro_doc` so it re-runs on document load; guarded per
/// payload-hash so it fires at most once per document. Gated by
/// `authorize_auto_run` (trusted + `auto_run_open`); the runner re-checks the
/// token, so nothing fires without the flag.
pub(super) fn use_auto_run_effect(
    ctx: MacroCtx,
    svc: MacroService,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut launch: RunnerLaunch,
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
                    launch.view.set(Some(v));
                    launch.auto.set(true);
                }
            }
        }
    });
}

/// Dispatches a MACROBUTTON click (spec §6): when the document's macros are
/// enabled, opens the runner on the named procedure through the gated path; when
/// disabled, prompts to enable first — a click never runs a macro from an
/// untrusted document.
pub(super) fn use_click_dispatch_effect(
    ctx: MacroCtx,
    svc: MacroService,
    mut macro_run_request: Signal<Option<String>>,
    mut launch: RunnerLaunch,
    mut trust_open: Signal<bool>,
) {
    use_effect(move || {
        if let Some(name) = macro_run_request() {
            if let Some(payload) = payload_of(&ctx.0) {
                if svc.decision_for(&payload).is_enabled() {
                    let v = super::editor_macro_extract::extract_view(&payload);
                    if !v.modules.is_empty() {
                        launch.auto.set(false);
                        launch.proc.set(Some(name));
                        launch.view.set(Some(v));
                    }
                } else {
                    trust_open.set(true);
                }
            }
            macro_run_request.set(None);
        }
    });
}
