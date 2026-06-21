// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Off-main-thread first layout for the document-open path.
//!
//! The first paginated layout (`seed_layout_from_document`) is the dominant
//! cost of opening a document — tens of ms on a multi-page file, because the
//! shared `FontResources` shaping/glyph caches are cold. Running it inline on
//! the main thread blocks the frame, so the loading indicator never paints and
//! the open appears to freeze.
//!
//! [`compute_layout_off_main_thread`] moves that work onto a short-lived worker
//! thread and hands the result back through a `oneshot` channel. The `.await`
//! on the channel is a genuine cross-thread yield (the same mechanism the
//! editor already uses for `MountedData::get_client_rect().await`), so the main
//! thread stays free to paint the loading indicator and remains responsive
//! while the layout runs.
//!
//! Both [`loki_layout::FontResources`] and [`loki_layout::DocumentLayout`] are
//! `Send`, so the shared font context can be locked and driven on the worker
//! and the finished layout returned across the thread boundary. Because the
//! worker locks the *shared* `FontResources`, the cache is warmed as a side
//! effect: the first on-main-thread relayout (a keystroke) is already warm.

use std::sync::{Arc, Mutex};

use futures_channel::oneshot;
use loki_doc_model::document::Document;

use crate::editing::relayout::LaidOut;
use crate::editing::state::{DocumentState, compute_seed_layout};

/// Lays out `doc` on a worker thread and resolves to `(doc, layout)`.
///
/// `doc` is moved to the worker and returned with the computed layout so the
/// caller can both publish the layout and seed the Loro CRDT without cloning
/// the document a second time. Resolves to `None` if the worker could not be
/// spawned or was dropped before it finished (e.g. the tab closed mid-open).
///
/// The caller is responsible for publishing the result
/// ([`crate::editing::state::publish_seed_layout`]) on the main thread and for
/// re-checking that the document is still the active one before doing so.
pub(super) async fn compute_layout_off_main_thread(
    doc_state: Arc<Mutex<DocumentState>>,
    doc: Document,
) -> Option<(Document, LaidOut)> {
    let fr_arc = {
        let state = doc_state.lock().ok()?;
        state.shared_font_resources.clone()
    };
    let (tx, rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("loki-open-layout".into())
        .spawn(move || {
            let layout = compute_seed_layout(&fr_arc, &doc);
            // A dropped receiver (tab closed mid-open) makes this a no-op; the
            // computed layout is simply discarded.
            let _ = tx.send((doc, layout));
        })
        .ok()?;
    rx.await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editing::state::publish_seed_layout;

    /// The open path lays out on a worker thread and publishes on the main one.
    /// This exercises both halves and — by moving `FontResources` into the
    /// worker and the `DocumentLayout` back out — fails to compile if either
    /// type regresses to `!Send`, which is the invariant the feature relies on.
    #[test]
    fn worker_layout_publishes_to_doc_state() {
        let doc_state = Arc::new(Mutex::new(DocumentState::new()));
        let fonts = doc_state
            .lock()
            .expect("lock doc_state")
            .shared_font_resources
            .clone();

        let doc = Document::new_blank();
        // Compile-checks Send: `fonts` moves in, `DocumentLayout` moves out.
        let layout = std::thread::spawn(move || compute_seed_layout(&fonts, &doc))
            .join()
            .expect("worker thread panicked");

        let doc = Document::new_blank();
        let pages = publish_seed_layout(&doc_state, &doc, layout);
        assert!(pages >= 1, "a blank document lays out to at least one page");

        let state = doc_state.lock().expect("lock doc_state");
        assert_eq!(state.page_count, pages);
        assert!(state.paginated_layout.is_some());
        assert!(state.document.is_some());
    }
}
