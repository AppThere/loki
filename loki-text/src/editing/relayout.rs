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

/// Layout options for the editor: retain Parley layouts for hit-testing/cursor,
/// plus the active spell checker (if any) so misspelled words get squiggles.
fn edit_opts() -> LayoutOptions {
    LayoutOptions {
        preserve_for_editing: true,
        spell: crate::editing::spell::active(),
        ..Default::default()
    }
}

/// A freshly computed paginated layout plus the reuse metadata to store.
pub(crate) struct LaidOut {
    pub layout: PaginatedLayout,
    pub reuse: PaginatedReuse,
    /// Font substitutions this run touched (requested → substitute). An
    /// under-count on incremental runs (cache hits skip resolution) — callers
    /// extend `DocumentState::font_substitutions` rather than replace it.
    pub substitutions: std::collections::HashMap<String, Option<String>>,
    /// Whether the incremental path produced this layout (`false` = a full
    /// pass ran).
    ///
    /// Reported by the `loki_text::mem` counters because the two paths visit
    /// very different fractions of the document: a full pass looks up *every*
    /// live paragraph in the shaping cache, an incremental one only the
    /// re-flowed span. Which of those is the steady state during typing decides
    /// what can bound that cache, so it is measured rather than assumed.
    pub incremental: bool,
}

/// Lays out `doc` (paginated), reusing `prev` incrementally when the edit is
/// eligible. `prev` is `(previous document, previous layout, previous reuse)`;
/// pass `None` for a fresh document open.
pub(crate) fn relayout_paginated(
    fr: &mut FontResources,
    doc: &Document,
    prev: Option<(&Document, &PaginatedLayout, &PaginatedReuse)>,
) -> LaidOut {
    let opts = edit_opts();
    fr.begin_substitution_run();
    if let Some((prev_doc, prev_layout, prev_reuse)) = prev
        && let Some((layout, reuse)) =
            relayout_paginated_incremental(fr, doc, prev_doc, prev_layout, prev_reuse, 1.0, &opts)
    {
        let substitutions = fr.take_substitution_run();
        return LaidOut {
            layout,
            reuse,
            substitutions,
            incremental: true,
        };
    }
    let (layout, reuse) = layout_paginated_full(fr, doc, 1.0, &opts);
    let substitutions = fr.take_substitution_run();
    LaidOut {
        layout,
        reuse,
        substitutions,
        incremental: false,
    }
}

/// Page count and CSS-pixel page dimensions (points × 96/72) for a layout.
///
/// When any page carries comment-panel items, the width is widened by the
/// comment gutter so the panel to the right of the page is reachable.
pub(crate) fn page_metrics(layout: &PaginatedLayout) -> (usize, f32, f32) {
    let has_comments = layout.pages.iter().any(|p| !p.comment_items.is_empty());
    let mut width_pt = layout.page_size.width;
    if has_comments {
        width_pt += loki_layout::COMMENT_GUTTER_WIDTH;
    }
    let w = width_pt * (96.0 / 72.0);
    let h = layout.page_size.height * (96.0 / 72.0);
    (layout.pages.len(), w, h)
}
