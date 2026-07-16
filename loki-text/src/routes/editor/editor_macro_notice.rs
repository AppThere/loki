// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Macro-present notice + trust/security entry points (macro spec §9.1, §9.4,
//! §9.6).
//!
//! When an opened document carries a preserved macro payload
//! (`document.source.macros`), the editor shows a passive, non-modal infobar.
//! Macros are **disabled by default** for documents the user did not author
//! (spec §2). From the infobar the user can open the trust dialog
//! ([`AtMacroTrustDialog`]) to enable them, view the source read-only, or (once
//! enabled) open the Document Security panel to manage capability grants.
//!
//! There is still **no execution surface** — Phase 4 records the *decision*
//! (via the ambient [`MacroService`]); Phase 5 wires actual execution. Nothing
//! here runs a macro.
//!
//! Source extraction for the viewer is on-demand; the per-frame cost of the
//! infobar is a cheap presence check, and this component is memoised on the
//! document-state `Arc` (see [`MacroCtx`]) so it re-renders only on macro
//! interactions, not on every keystroke.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtInfobar, AtMacroTrustDialog, MacroTrustChoice};
use dioxus::prelude::*;
use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};
use loki_i18n::fl;
use loki_macro_host::{MacroService, TrustDecision};

use crate::editing::state::DocumentState;

/// A cheap-to-compare handle to the editor's document state, usable as a Dioxus
/// prop (equality is `Arc` identity, so it never spuriously re-runs children).
#[derive(Clone)]
pub(super) struct MacroCtx(pub(super) Arc<Mutex<DocumentState>>);

impl PartialEq for MacroCtx {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// One module's read-only source for the viewer.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ViewerModule {
    pub(super) name: String,
    pub(super) source: String,
}

/// The extracted macro view: modules plus an optional tamper warning.
#[derive(Clone, PartialEq, Eq, Default)]
pub(super) struct MacroView {
    pub(super) modules: Vec<ViewerModule>,
    pub(super) tamper: Option<String>,
}

/// Clones out the macro payload and document title under a single short lock, or
/// `None` when no macro-carrying document is loaded.
fn read_doc(doc_state: &Arc<Mutex<DocumentState>>) -> Option<(MacroPayload, String)> {
    let guard = doc_state.try_lock().ok()?;
    let doc = guard.document.as_ref()?;
    let payload = doc.source.as_ref()?.macros.as_ref()?.clone();
    if payload.is_empty() {
        return None;
    }
    let title = doc
        .meta
        .title
        .clone()
        .unwrap_or_else(|| fl!("macros-viewer-title"));
    Some((payload, title))
}

/// A human name for the macro project, derived from the payload family.
fn project_name(payload: &MacroPayload) -> String {
    match payload.kind {
        MacroPayloadKind::OoxmlVba => "VBA".to_string(),
        MacroPayloadKind::OdfBasic => "Basic".to_string(),
    }
}

/// Extracts the macro source for the viewer, dispatching by payload kind.
fn extract_view(payload: &MacroPayload) -> MacroView {
    match payload.kind {
        MacroPayloadKind::OoxmlVba => extract_vba(payload),
        MacroPayloadKind::OdfBasic => extract_basic(payload),
    }
}

fn extract_vba(payload: &MacroPayload) -> MacroView {
    let bytes = payload
        .parts
        .iter()
        .find(|p| p.name.ends_with("vbaProject.bin"))
        .map(|p| p.bytes.as_slice());
    match bytes.and_then(|b| loki_vba::VbaProject::read(b).ok()) {
        Some(project) => MacroView {
            modules: project
                .modules
                .into_iter()
                .map(|m| ViewerModule {
                    name: m.name,
                    source: m.source,
                })
                .collect(),
            tamper: project.tamper,
        },
        None => MacroView {
            modules: Vec::new(),
            tamper: Some(fl!("macros-viewer-unreadable")),
        },
    }
}

fn extract_basic(payload: &MacroPayload) -> MacroView {
    MacroView {
        modules: loki_odf::basic::extract_basic_modules(payload)
            .into_iter()
            .map(|m| ViewerModule {
                name: m.name,
                source: m.source,
            })
            .collect(),
        tamper: None,
    }
}

/// The passive macro infobar plus the trust dialog, Document Security panel, and
/// read-only viewer it can open. Always mounted; renders nothing unless the
/// document carries macros.
///
/// Touch targets: all controls meet the 44×44 logical-pixel minimum (WCAG
/// 2.5.8) via [`AtInfobar`] and the panel/dialog control sizing.
#[component]
pub(super) fn MacroNoticeBar(ctx: MacroCtx) -> Element {
    let svc = use_context::<MacroService>();
    let mut dismissed = use_signal(|| false);
    let mut view = use_signal(|| None::<MacroView>);
    let mut trust_open = use_signal(|| false);
    let mut panel_open = use_signal(|| false);

    let Some((payload, title)) = read_doc(&ctx.0) else {
        return rsx! {};
    };
    let decision = svc.decision_for(&payload);
    let enabled = decision.is_enabled();
    let project = project_name(&payload);

    // Infobar message + primary action depend on the current trust state.
    let (message, primary) = match decision {
        TrustDecision::Disabled => (fl!("macros-infobar-disabled"), fl!("macros-infobar-action")),
        TrustDecision::SessionOnly => (
            fl!("macros-security-state-session"),
            fl!("macros-security-open-action"),
        ),
        TrustDecision::Trusted => (
            fl!("macros-security-state-trusted"),
            fl!("macros-security-open-action"),
        ),
    };

    // Clones for the trust-dialog choice handler.
    let svc_choice = svc.clone();
    let payload_choice = payload.clone();
    let view_payload = payload.clone();

    rsx! {
        if !dismissed() {
            AtInfobar {
                message,
                action_label: primary,
                on_action: move |()| {
                    if enabled {
                        panel_open.set(true);
                    } else {
                        trust_open.set(true);
                    }
                },
                secondary_label: fl!("macros-view-action"),
                on_secondary: move |()| view.set(Some(extract_view(&view_payload))),
                dismiss_label: fl!("macros-infobar-dismiss"),
                on_dismiss: move |()| dismissed.set(true),
            }
        }

        if trust_open() {
            AtMacroTrustDialog {
                badge_label: fl!("macros-badge"),
                project_name: project.clone(),
                document_title: title.clone(),
                message: fl!("macros-trust-message"),
                keep_disabled_label: fl!("macros-trust-keep-disabled"),
                session_label: fl!("macros-trust-session"),
                trust_label: fl!("macros-trust-always"),
                on_choice: move |choice: MacroTrustChoice| {
                    apply_trust_choice(&svc_choice, &payload_choice, choice);
                    trust_open.set(false);
                },
            }
        }

        if panel_open() {
            super::editor_macro_security_panel::MacroSecurityPanel {
                payload: payload.clone(),
                title: title.clone(),
                on_close: move |()| panel_open.set(false),
            }
        }

        if let Some(v) = view() {
            super::editor_macro_viewer::MacroViewerPanel {
                view: v,
                on_close: move |()| view.set(None),
            }
        }
    }
}

/// Records the user's enable choice through the trust store (spec §2.3). A save
/// failure is logged but non-fatal — the decision still applies in-session.
fn apply_trust_choice(svc: &MacroService, payload: &MacroPayload, choice: MacroTrustChoice) {
    let result = match choice {
        MacroTrustChoice::KeepDisabled => svc.keep_disabled(payload, None),
        MacroTrustChoice::EnableSession => {
            svc.enable_session(payload);
            Ok(())
        }
        MacroTrustChoice::TrustAlways => svc.trust_document(payload, None),
    };
    if let Err(e) = result {
        tracing::warn!("macro trust save failed: {e}");
    }
}
