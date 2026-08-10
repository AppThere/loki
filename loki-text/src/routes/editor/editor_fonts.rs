// SPDX-License-Identifier: Apache-2.0

//! Font-related editor hooks and reads, split from `editor_inner` (300-line
//! ceiling budget).
//!
//! Both entry points are warm-up-aware: the shared [`FontResources`] handle
//! may still be scanning system fonts on its background thread when the editor
//! mounts, and nothing here may block the UI thread on that scan.
//!
//! [`FontResources`]: loki_layout::FontResources

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use super::editor_style_catalog::available_font_families;
use crate::editing::state::DocumentState;

/// Enumerates the available font families once per editor (system + bundled +
/// document-embedded), memoised for the style editor's font picker.
///
/// The enumeration blocks until the background font warm-up finishes, so it
/// runs on a worker thread and lands in the returned signal — the mount stays
/// cheap and the loading view can paint while a first-run system-font scan is
/// still going. Trade-off (unchanged from the synchronous version): faces
/// embedded after mount are not reflected until reopen.
pub(super) fn use_font_families(doc_state: &Arc<Mutex<DocumentState>>) -> Signal<Rc<Vec<String>>> {
    let mut font_families: Signal<Rc<Vec<String>>> = use_signal(|| Rc::new(Vec::new()));
    let ds_fonts = Arc::clone(doc_state);
    use_hook(move || {
        let fonts = ds_fonts
            .lock()
            .map(|s| s.shared_font_resources.clone())
            .unwrap_or_else(|e| e.into_inner().shared_font_resources.clone());
        let (tx, rx) = futures_channel::oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("loki-font-families".into())
            .spawn(move || {
                let _ = tx.send(available_font_families(&fonts));
            });
        if spawned.is_ok() {
            spawn(async move {
                if let Ok(names) = rx.await {
                    font_families.set(Rc::new(names));
                }
            });
        }
    });
    font_families
}

/// The font substitutions **this document's** layouts recorded (requested →
/// substitute), for the status-bar chip and the detail panel.
///
/// Reads `DocumentState::font_substitutions` — the per-document accumulator —
/// not `FontResources::substitutions`, which is a process-lifetime resolve
/// memo spanning every document this app ever laid out. Reading the memo here
/// is what made the chip survive tab switches and tab closes.
pub(super) fn font_substitutions(
    doc_state: &Arc<Mutex<DocumentState>>,
) -> HashMap<String, Option<String>> {
    doc_state
        .lock()
        .ok()
        .map(|s| s.font_substitutions.clone())
        .unwrap_or_default()
}
