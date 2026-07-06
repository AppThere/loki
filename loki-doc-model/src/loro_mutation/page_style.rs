// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-**page-style** geometry mutation (Spec 05 M6 page family, ADR-0012).
//!
//! The document's `set_document_*` page mutations ([`super::page`]) apply to
//! *every* section — the "change the whole document" model. This applies a page
//! layout to a **specific set of sections** instead: the sections that belong to
//! one named page style (LibreOffice's model — editing a page style changes only
//! the pages that use it). The panel computes the target section indices from the
//! derived page-style grouping (`section_page_style_ids`) and passes them here.
//!
//! It writes the whole geometry the page inspector shows — size, orientation,
//! margins (t/b/l/r), and columns — leaving header/footer/gutter distances and
//! page numbering untouched (the panel does not edit them).

use loro::{LoroDoc, LoroMap};

use super::MutationError;
use crate::layout::page::{PageLayout, PageOrientation};
use crate::loro_schema::{
    KEY_COL_COUNT, KEY_COL_GAP, KEY_COL_SEPARATOR, KEY_COLUMNS, KEY_LAYOUT, KEY_MARGIN_BOTTOM,
    KEY_MARGIN_LEFT, KEY_MARGIN_RIGHT, KEY_MARGIN_TOP, KEY_MARGINS, KEY_ORIENTATION, KEY_PAGE_SIZE,
    KEY_SECTIONS,
};

/// Reads a nested `LoroMap` child by key.
fn child_map(map: &LoroMap, key: &str) -> Option<LoroMap> {
    map.get(key)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
}

/// Gets the child map at `key`, creating an empty one if absent.
fn child_map_or_create(map: &LoroMap, key: &str) -> Result<LoroMap, MutationError> {
    match child_map(map, key) {
        Some(m) => Ok(m),
        None => Ok(map.insert_container(key, LoroMap::new())?),
    }
}

/// Applies `layout`'s geometry (size, orientation, margins, columns) to each
/// section in `section_indices`, leaving every other section — and this
/// section's headers/footers/gutter/page-numbering — untouched. Out-of-range or
/// malformed section indices are skipped.
///
/// This is the per-page-style editing primitive: the caller passes the sections
/// that belong to the edited page style, so only those pages change.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn set_page_style_geometry(
    loro: &LoroDoc,
    section_indices: &[usize],
    layout: &PageLayout,
) -> Result<(), MutationError> {
    let sections = loro.get_list(KEY_SECTIONS);
    for &s in section_indices {
        let Some(section) = sections
            .get(s)
            .and_then(|v| v.into_container().ok())
            .and_then(|c| c.into_map().ok())
        else {
            continue;
        };
        let lay = child_map_or_create(&section, KEY_LAYOUT)?;

        // Page size.
        let size = child_map_or_create(&lay, KEY_PAGE_SIZE)?;
        size.insert("width", layout.page_size.width.value())?;
        size.insert("height", layout.page_size.height.value())?;

        // Orientation flag (the layout engine reads the effective size directly).
        lay.insert(
            KEY_ORIENTATION,
            match layout.orientation {
                PageOrientation::Landscape => "Landscape",
                PageOrientation::Portrait => "Portrait",
            },
        )?;

        // Margins (t/b/l/r only — header/footer/gutter preserved).
        let margins = child_map_or_create(&lay, KEY_MARGINS)?;
        margins.insert(KEY_MARGIN_TOP, layout.margins.top.value())?;
        margins.insert(KEY_MARGIN_BOTTOM, layout.margins.bottom.value())?;
        margins.insert(KEY_MARGIN_LEFT, layout.margins.left.value())?;
        margins.insert(KEY_MARGIN_RIGHT, layout.margins.right.value())?;

        // Columns: count (≥1), plus gap/separator when a multi-column layout.
        let cols = child_map_or_create(&lay, KEY_COLUMNS)?;
        let count = layout.columns.as_ref().map_or(1, |c| c.count).max(1);
        cols.insert(KEY_COL_COUNT, i64::from(count))?;
        if let Some(c) = layout.columns.as_ref() {
            cols.insert(KEY_COL_GAP, c.gap.value())?;
            cols.insert(KEY_COL_SEPARATOR, c.separator)?;
        }
    }
    Ok(())
}
