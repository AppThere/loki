// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Incremental re-layout for single-paragraph edits.
//!
//! # Algorithm
//!
//! When the user types or deletes a character in paragraph P:
//!
//! 1. **Re-layout P** — call [`layout_paragraph`] on the updated paragraph text
//!    from the new [`Document`] snapshot.
//!
//! 2. **Same-height fast path** — compare the new paragraph height to the old
//!    height stored in the matching [`PageEditingData`] entry.  If the heights
//!    are within 0.5 pt of each other:
//!    a. Clone the existing [`PaginatedLayout`].
//!    b. Replace `editing_data.paragraph_layouts[para_idx]` with the new layout.
//!    c. Rebuild the `content_items` for the affected paragraph in-place (glyphs
//!       and decorations from the new [`ParagraphLayout`]).
//!    d. Return [`IncrementalLayoutResult::FastPath`] with the patched layout.
//!
//! 3. **Full-section fallback** — height changed; the caller must discard the
//!    old layout and call [`crate::layout_document`] to re-layout the whole
//!    document.  Return [`IncrementalLayoutResult::FullRelayout`].
//!
//! # Current status
//!
//! TODO(incremental-flow): The fast path in step 2 requires a stable mapping
//! from `(section_index, block_index)` to `(page_index, paragraph_index)`.
//! This mapping is not yet stored in [`PageEditingData`] — the Vec of
//! `paragraph_origins` is ordered by layout order but does not carry block IDs.
//! Until `PageEditingData` is extended with block identifiers, this function
//! always returns [`IncrementalLayoutResult::FullRelayout`], delegating to the
//! existing full-pipeline path in `LokiDocumentSource::render()`.

use std::sync::Arc;

use loki_doc_model::Document;

use crate::result::PaginatedLayout;
use crate::{FontResources, LayoutOptions};

/// The result of an incremental layout attempt.
#[derive(Debug)]
pub enum IncrementalLayoutResult {
    /// The changed paragraph had the same line height as before.
    ///
    /// The inner value is a patched clone of the original [`PaginatedLayout`]
    /// with the affected [`PageEditingData::paragraph_layouts`] entry updated
    /// and content items regenerated for the changed paragraph.
    ///
    /// The caller should update `DocumentState::paginated_layout` and bump
    /// `DocumentState::generation` so that `LokiDocumentSource::render()` repaints
    /// without a full re-layout.
    FastPath(Arc<PaginatedLayout>),

    /// The changed paragraph's height changed, or the fast path was not
    /// applicable (see module-level TODO).
    ///
    /// The caller should call [`crate::layout_document`] to re-layout the full
    /// document.
    FullRelayout,
}

/// Attempt an incremental re-layout for a single changed paragraph.
///
/// # Parameters
///
/// * `resources` — font resources shared with the full layout pipeline.
/// * `doc` — the updated document snapshot derived from the live [`LoroDoc`]
///   after the mutation was applied.
/// * `section_index` — zero-based index of the section containing the changed block.
/// * `block_index` — zero-based index of the changed block within its section.
/// * `old_layout` — the [`PaginatedLayout`] produced by the previous
///   [`crate::layout_document`] call (still valid for all unchanged paragraphs).
/// * `display_scale` — passed through to [`crate::layout_paragraph`].
/// * `options` — layout options; must have `preserve_for_editing: true` when
///   the result will be used for cursor hit-testing.
///
/// # Returns
///
/// [`IncrementalLayoutResult::FastPath`] when the height-preserving fast path
/// succeeds, [`IncrementalLayoutResult::FullRelayout`] otherwise.
pub fn incremental_layout(
    _resources: &mut FontResources,
    _doc: &Document,
    _section_index: usize,
    _block_index: usize,
    _old_layout: &PaginatedLayout,
    _display_scale: f32,
    _options: &LayoutOptions,
) -> IncrementalLayoutResult {
    // TODO(incremental-flow): implement the fast path once PageEditingData
    // carries stable block identifiers alongside paragraph_origins so that
    // (section_index, block_index) can be mapped to (page_index, para_idx).
    //
    // Fast path outline (when block IDs are available):
    //   1. Find (page_idx, para_idx) from block ID lookup.
    //   2. Resolve para props for the block (resolve_para_props / flatten_paragraph).
    //   3. Re-layout the paragraph: layout_paragraph(&mut resources, text, spans,
    //        &resolved, content_width, display_scale, options.preserve_for_editing).
    //   4. Compare new height to old_layout.pages[page_idx]
    //        .editing_data.paragraph_layouts[para_idx].height.
    //   5. If |new_height - old_height| <= 0.5:
    //        a. Clone old_layout into a new Arc<PaginatedLayout>.
    //        b. Replace paragraph_layouts[para_idx] = Some(Arc::new(new_para)).
    //        c. Rebuild content_items for the paragraph slice.
    //        d. Return FastPath(Arc::new(patched_layout)).
    //   6. Else return FullRelayout.
    IncrementalLayoutResult::FullRelayout
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_relayout_variant_is_matchable() {
        let r = IncrementalLayoutResult::FullRelayout;
        assert!(matches!(r, IncrementalLayoutResult::FullRelayout));
    }

    #[test]
    fn fast_path_variant_holds_arc() {
        let pl = PaginatedLayout {
            page_size: crate::geometry::LayoutSize::new(595.0, 842.0),
            pages: vec![],
        };
        let r = IncrementalLayoutResult::FastPath(Arc::new(pl));
        assert!(matches!(r, IncrementalLayoutResult::FastPath(_)));
    }

    #[test]
    fn incremental_layout_returns_full_relayout_for_now() {
        let mut resources = FontResources::new();
        let doc = loki_doc_model::Document::new();
        let old_layout = PaginatedLayout {
            page_size: crate::geometry::LayoutSize::new(595.0, 842.0),
            pages: vec![],
        };
        let result = incremental_layout(
            &mut resources,
            &doc,
            0,
            0,
            &old_layout,
            1.0,
            &LayoutOptions::default(),
        );
        assert!(matches!(result, IncrementalLayoutResult::FullRelayout));
    }
}
