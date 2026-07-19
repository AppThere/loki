// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! On-demand macro-source extraction for the viewer / runner (split from
//! `editor_macro_notice` for the 300-line ceiling).
//!
//! VBA source is read via `loki-vba` (source only, never p-code); `StarBasic`
//! via `loki_odf::basic`. A malformed VBA project degrades to an empty view with
//! the "unreadable" tamper note.

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};
use loki_i18n::fl;

use super::editor_macro_notice::{MacroView, ViewerModule};

/// Extracts the macro source for the viewer/runner, dispatching by payload kind.
pub(super) fn extract_view(payload: &MacroPayload) -> MacroView {
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
