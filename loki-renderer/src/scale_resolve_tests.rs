// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The ordering `resolve` exists to hold — asserted where it broke.

use std::sync::Arc;

use appthere_canvas::residency::TextureBudget;
use loki_doc_model::document::Document;
use loki_layout::{LayoutSize, PaginatedLayout};

use super::{ScaleInputs, resolve};
use crate::ViewMode;
use crate::doc_page_source::DocPageSource;
use crate::render_layout::RenderLayout;

fn paginated_inputs() -> ScaleInputs {
    ScaleInputs {
        view_mode: ViewMode::Paginated,
        reflow_width_px: 0.0,
        zoom: 1.0,
        device_scale_factor: 2.0,
        texture_budget: TextureBudget::baseline(),
    }
}

/// An empty layout, standing in for the one the editor computed. Its contents do
/// not matter — its **identity** does, which is the whole assertion.
fn editor_layout(width: f32) -> Arc<PaginatedLayout> {
    Arc::new(PaginatedLayout {
        page_size: LayoutSize::new(width, 792.0),
        pages: Vec::new(),
    })
}

/// Whichever layout `source` currently holds, as a pointer to compare.
fn cached(source: &DocPageSource) -> Option<Arc<PaginatedLayout>> {
    let guard = source.layout_for_generation(source.current_generation());
    match guard.as_ref() {
        Some((_, RenderLayout::Paginated(pl))) => Some(pl.clone()),
        _ => None,
    }
}

/// **The editor's layout must survive the frame, and the defect was an ordering
/// nothing checked.**
///
/// `provide_paginated_layout` is a no-op once the cache holds the current
/// generation, so any read that misses the cache first causes the renderer to lay
/// the document out itself — and the editor's layout is then dropped on the
/// floor. That is exactly what the capability call did when it landed ahead of
/// the seeding: a second full document layout every generation, and the ~20 MB
/// system-font scan on open, with no test failing and no symptom but latency.
///
/// Asserted by **identity**, not by shape: a recomputed layout of the same
/// document would compare equal on every field that matters and still be the
/// wrong object. `Arc::ptr_eq` is the only instrument that can tell "reused" from
/// "recomputed to look the same".
#[test]
fn the_editors_layout_is_seeded_before_anything_can_read_one() {
    let source = Arc::new(DocPageSource::new(Arc::new(Document::default())));
    let provided = editor_layout(612.0);
    resolve(&source, paginated_inputs(), Some(provided.clone()));
    let held = cached(&source).expect("a paginated layout must be cached");
    assert!(
        Arc::ptr_eq(&held, &provided),
        "the renderer laid the document out itself and discarded the editor's \
         layout — the capability call is reading page sizes before the seeding",
    );
}

/// **The polarity: a new generation adopts the new layout.**
///
/// Without this the test above passes for a `resolve` that seeds once and never
/// again — which would pin every later frame to the layout of the document as it
/// was when it opened, and every edit would render stale.
#[test]
fn a_later_generation_adopts_the_layout_provided_with_it() {
    let source = Arc::new(DocPageSource::new(Arc::new(Document::default())));
    let first = editor_layout(612.0);
    resolve(&source, paginated_inputs(), Some(first.clone()));

    // A document mutation: `update_doc` bumps the generation and clears the
    // cache, which is what makes the next seeding land.
    source.update_doc(Arc::new(Document::default()));
    let second = editor_layout(1191.0);
    resolve(&source, paginated_inputs(), Some(second.clone()));

    let held = cached(&source).expect("a paginated layout must be cached");
    assert!(
        Arc::ptr_eq(&held, &second),
        "a new generation kept the previous layout: {:?}",
        held.page_size,
    );
    assert!(!Arc::ptr_eq(&held, &first));
}
