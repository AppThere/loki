// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::DocPageSource`]'s single-canonical-layout reuse.

use std::sync::Arc;

use loki_doc_model::document::Document;
use loki_layout::{DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout};

use super::DocPageSource;
use crate::render_layout::RenderLayout;

/// Lays out a blank document in paginated mode, returning a shared layout.
fn paginated(doc: &Document) -> Arc<PaginatedLayout> {
    let mut fr = FontResources::new();
    match loki_layout::layout_document(
        &mut fr,
        doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions {
            preserve_for_editing: true,
            spell: None,
            ..Default::default()
        },
    ) {
        DocumentLayout::Paginated(pl) => Arc::new(pl),
        _ => panic!("paginated mode must return a paginated layout"),
    }
}

/// A provided layout is reused by `layout_for_generation` (same `Arc`, i.e. no
/// recompute) for the current generation.
#[test]
fn provided_layout_is_reused() {
    let doc = Arc::new(Document::new_blank());
    let source = DocPageSource::new(doc.clone());
    let provided = paginated(&doc);

    source.provide_paginated_layout(provided.clone());

    let cur_gen = source.current_generation();
    let guard = source.layout_for_generation(cur_gen);
    let Some((g, RenderLayout::Paginated(got))) = guard.as_ref() else {
        panic!("expected a cached paginated layout");
    };
    assert_eq!(*g, cur_gen);
    assert!(
        Arc::ptr_eq(got, &provided),
        "renderer should reuse the provided layout, not recompute"
    );
}

/// After the document changes, the stale provided layout is dropped and the
/// renderer computes a fresh one (a different allocation).
#[test]
fn doc_change_invalidates_provided_layout() {
    let doc = Arc::new(Document::new_blank());
    let source = DocPageSource::new(doc.clone());
    let provided = paginated(&doc);
    source.provide_paginated_layout(provided.clone());

    // A new document pointer advances the generation and clears the cache.
    source.update_doc(Arc::new(Document::new_blank()));

    let cur_gen = source.current_generation();
    let guard = source.layout_for_generation(cur_gen);
    let Some((_, RenderLayout::Paginated(got))) = guard.as_ref() else {
        panic!("expected a recomputed paginated layout");
    };
    assert!(
        !Arc::ptr_eq(got, &provided),
        "stale provided layout must not survive a document change"
    );
}

/// Providing a layout when the cache already holds the current generation is a
/// no-op (the renderer keeps the layout it already has).
#[test]
fn provide_is_noop_when_generation_already_cached() {
    let doc = Arc::new(Document::new_blank());
    let source = DocPageSource::new(doc.clone());

    // Force a compute for the current generation.
    let first = paginated(&doc);
    source.provide_paginated_layout(first.clone());
    let cur_gen = source.current_generation();

    // A second provide for the same generation should be ignored.
    let second = paginated(&doc);
    source.provide_paginated_layout(second.clone());

    let guard = source.layout_for_generation(cur_gen);
    let Some((_, RenderLayout::Paginated(got))) = guard.as_ref() else {
        panic!("expected a cached paginated layout");
    };
    assert!(
        Arc::ptr_eq(got, &first),
        "first provided layout for a generation wins"
    );
}
