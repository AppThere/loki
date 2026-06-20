// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paginated relayout shared by the document-open and per-mutation paths.
//!
//! [`relayout_paginated`] reuses the previous layout incrementally when the edit
//! is eligible (see `loki_layout::incremental`), falling back to a full layout
//! otherwise. Both paths return the [`PaginatedReuse`] metadata the *next* edit
//! needs, so incremental reuse chains across keystrokes.

use loki_doc_model::document::Document;
use loki_layout::{
    FontResources, LayoutOptions, PaginatedLayout, PaginatedReuse, layout_paginated_full,
    relayout_paginated_incremental,
};

/// Layout options for the editor: retain Parley layouts for hit-testing/cursor.
const EDIT_OPTS: LayoutOptions = LayoutOptions {
    preserve_for_editing: true,
};

/// A freshly computed paginated layout plus the reuse metadata to store.
pub(crate) struct LaidOut {
    pub layout: PaginatedLayout,
    pub reuse: PaginatedReuse,
}

/// Lays out `doc` (paginated), reusing `prev` incrementally when the edit is
/// eligible. `prev` is `(previous document, previous layout, previous reuse)`;
/// pass `None` for a fresh document open.
pub(crate) fn relayout_paginated(
    fr: &mut FontResources,
    doc: &Document,
    prev: Option<(&Document, &PaginatedLayout, &PaginatedReuse)>,
) -> LaidOut {
    if let Some((prev_doc, prev_layout, prev_reuse)) = prev
        && let Some((layout, reuse)) = relayout_paginated_incremental(
            fr,
            doc,
            prev_doc,
            prev_layout,
            prev_reuse,
            1.0,
            &EDIT_OPTS,
        )
    {
        return LaidOut { layout, reuse };
    }
    let (layout, reuse) = layout_paginated_full(fr, doc, 1.0, &EDIT_OPTS);
    LaidOut { layout, reuse }
}

/// Page count and CSS-pixel page dimensions (points × 96/72) for a layout.
pub(crate) fn page_metrics(layout: &PaginatedLayout) -> (usize, f32, f32) {
    let w = layout.page_size.width * (96.0 / 72.0);
    let h = layout.page_size.height * (96.0 / 72.0);
    (layout.pages.len(), w, h)
}
