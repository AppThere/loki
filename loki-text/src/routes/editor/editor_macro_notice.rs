// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Macro-present notice + read-only viewer entry point (macro spec §9.1, §9.6).
//!
//! When an opened document carries a preserved macro payload
//! (`document.source.macros`), the editor shows a passive, non-modal infobar:
//! macros are **disabled** (there is still no execution surface — that arrives
//! with `loki-macro-host`), but the user can **view** the macro source
//! read-only to see what the document would run before making any trust
//! decision ("visibility before executability").
//!
//! Source extraction is on-demand (only when the viewer is opened), so the
//! per-frame cost of the infobar is just a cheap presence check. VBA source is
//! read via `loki-vba` (source only, never p-code); `StarBasic` via
//! `loki_odf::basic`.

use std::sync::{Arc, Mutex};

use appthere_ui::AtInfobar;
use dioxus::prelude::*;
use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};
use loki_i18n::fl;

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

/// Returns `true` if the currently-loaded document carries a preserved macro
/// payload. Non-blocking (a contended lock reports "none this frame").
pub(super) fn macros_present(doc_state: &Arc<Mutex<DocumentState>>) -> bool {
    doc_state
        .try_lock()
        .ok()
        .and_then(|s| {
            s.document
                .as_ref()
                .and_then(|d| d.source.as_ref())
                .map(|src| src.macros.as_ref().is_some_and(|m| !m.is_empty()))
        })
        .unwrap_or(false)
}

/// Extracts the macro source for the viewer, dispatching by payload kind.
fn extract_view(doc_state: &Arc<Mutex<DocumentState>>) -> MacroView {
    let Ok(guard) = doc_state.try_lock() else {
        return MacroView::default();
    };
    let Some(payload) = guard
        .document
        .as_ref()
        .and_then(|d| d.source.as_ref())
        .and_then(|src| src.macros.as_ref())
    else {
        return MacroView::default();
    };
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

/// The passive "this document contains macros" infobar, with a "View macros…"
/// action that opens the read-only source viewer. Always mounted; renders
/// nothing unless the document carries macros.
///
/// Touch targets: the infobar's action and dismiss controls meet the 44×44
/// logical-pixel minimum (WCAG 2.5.8) via [`AtInfobar`]'s control sizing.
#[component]
pub(super) fn MacroNoticeBar(ctx: MacroCtx) -> Element {
    let mut dismissed = use_signal(|| false);
    let mut view = use_signal(|| None::<MacroView>);

    if !macros_present(&ctx.0) {
        return rsx! {};
    }

    let open_ctx = ctx.0.clone();
    rsx! {
        if !dismissed() {
            AtInfobar {
                message: fl!("macros-infobar-disabled"),
                action_label: fl!("macros-view-action"),
                on_action: move |()| view.set(Some(extract_view(&open_ctx))),
                dismiss_label: fl!("macros-infobar-dismiss"),
                on_dismiss: move |()| dismissed.set(true),
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
