// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Page-layout mutations: orientation (portrait ↔ landscape).
//!
//! The layout engine reads a section's **effective** `page_size` directly
//! (`flow.rs` uses `page_size.width`/`.height` as-is); [`PageOrientation`] is a
//! metadata flag. So toggling orientation swaps `width` ↔ `height` on every
//! section's page-size map *and* updates the orientation flag, keeping the two
//! consistent. Applying to every section keeps the whole document uniform (the
//! common single-section case, and a sensible default for multi-section docs).
//!
//! [`PageOrientation`]: crate::layout::page::PageOrientation

use loro::{LoroDoc, LoroMap};

use super::MutationError;
use crate::loro_schema::{KEY_LAYOUT, KEY_ORIENTATION, KEY_PAGE_SIZE, KEY_SECTIONS};

/// Reads a nested `LoroMap` child by key.
fn child_map(map: &LoroMap, key: &str) -> Option<LoroMap> {
    map.get(key)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
}

/// Reads an `f64` value from a map (`0.0` when absent).
fn read_f64(map: &LoroMap, key: &str) -> f64 {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_double().ok())
        .unwrap_or(0.0)
}

/// Whether the document is currently landscape — section 0's page is wider than
/// it is tall. `false` (portrait) when there is no measurable page.
#[must_use]
pub fn document_is_landscape(loro: &LoroDoc) -> bool {
    let sections = loro.get_list(KEY_SECTIONS);
    let Some(size) = sections
        .get(0)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|section| child_map(&section, KEY_LAYOUT))
        .and_then(|layout| child_map(&layout, KEY_PAGE_SIZE))
    else {
        return false;
    };
    read_f64(&size, "width") > read_f64(&size, "height")
}

/// Sets every section's page orientation to `landscape` (or portrait when
/// `false`): swaps `width` ↔ `height` when the current page does not already
/// match, and writes the orientation flag. Idempotent.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn set_document_orientation(loro: &LoroDoc, landscape: bool) -> Result<(), MutationError> {
    let sections = loro.get_list(KEY_SECTIONS);
    for s in 0..sections.len() {
        let Some(section) = sections
            .get(s)
            .and_then(|v| v.into_container().ok())
            .and_then(|c| c.into_map().ok())
        else {
            continue;
        };
        let Some(layout) = child_map(&section, KEY_LAYOUT) else {
            continue;
        };
        if let Some(size) = child_map(&layout, KEY_PAGE_SIZE) {
            let w = read_f64(&size, "width");
            let h = read_f64(&size, "height");
            let currently_landscape = w > h;
            if w > 0.0 && h > 0.0 && currently_landscape != landscape {
                size.insert("width", h)?;
                size.insert("height", w)?;
            }
        }
        layout.insert(
            KEY_ORIENTATION,
            if landscape { "Landscape" } else { "Portrait" },
        )?;
    }
    Ok(())
}
