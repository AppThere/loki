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
use crate::loro_schema::{
    KEY_COL_COUNT, KEY_COL_GAP, KEY_COL_SEPARATOR, KEY_COLUMNS, KEY_LAYOUT, KEY_MARGIN_BOTTOM,
    KEY_MARGIN_LEFT, KEY_MARGIN_RIGHT, KEY_MARGIN_TOP, KEY_MARGINS, KEY_ORIENTATION, KEY_PAGE_SIZE,
    KEY_SECTIONS,
};

/// Default gap between columns when the Layout tab first creates them: 0.5 in.
const DEFAULT_COL_GAP_PT: f64 = 36.0;

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

/// Section 0's page margins in points as `(top, bottom, left, right)`, or `None`
/// when there is no margins map. Lets the Layout tab highlight the active
/// margin preset.
#[must_use]
pub fn document_margins(loro: &LoroDoc) -> Option<(f64, f64, f64, f64)> {
    let sections = loro.get_list(KEY_SECTIONS);
    let margins = sections
        .get(0)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|section| child_map(&section, KEY_LAYOUT))
        .and_then(|layout| child_map(&layout, KEY_MARGINS))?;
    Some((
        read_f64(&margins, KEY_MARGIN_TOP),
        read_f64(&margins, KEY_MARGIN_BOTTOM),
        read_f64(&margins, KEY_MARGIN_LEFT),
        read_f64(&margins, KEY_MARGIN_RIGHT),
    ))
}

/// Sets every section's top/bottom/left/right page margins (in points),
/// leaving header/footer/gutter distances untouched. Creates a margins map for
/// any section that lacks one.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn set_document_margins(
    loro: &LoroDoc,
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
) -> Result<(), MutationError> {
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
        let margins = match child_map(&layout, KEY_MARGINS) {
            Some(m) => m,
            None => layout.insert_container(KEY_MARGINS, LoroMap::new())?,
        };
        margins.insert(KEY_MARGIN_TOP, top)?;
        margins.insert(KEY_MARGIN_BOTTOM, bottom)?;
        margins.insert(KEY_MARGIN_LEFT, left)?;
        margins.insert(KEY_MARGIN_RIGHT, right)?;
    }
    Ok(())
}

/// Section 0's page size in points as `(width, height)`, or `None` when there
/// is no page-size map. Lets the Layout tab highlight the active size preset.
#[must_use]
pub fn document_page_size(loro: &LoroDoc) -> Option<(f64, f64)> {
    let sections = loro.get_list(KEY_SECTIONS);
    let size = sections
        .get(0)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|section| child_map(&section, KEY_LAYOUT))
        .and_then(|layout| child_map(&layout, KEY_PAGE_SIZE))?;
    Some((read_f64(&size, "width"), read_f64(&size, "height")))
}

/// Sets every section's page size to the paper of `portrait_w` × `portrait_h`
/// points, **preserving each section's orientation**: a landscape section keeps
/// the long edge as its width (so choosing "A4" while landscape gives A4
/// landscape, not portrait). Creates a page-size map for any section lacking one.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn set_document_page_size(
    loro: &LoroDoc,
    portrait_w: f64,
    portrait_h: f64,
) -> Result<(), MutationError> {
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
        let size = match child_map(&layout, KEY_PAGE_SIZE) {
            Some(m) => m,
            None => layout.insert_container(KEY_PAGE_SIZE, LoroMap::new())?,
        };
        let landscape = read_f64(&size, "width") > read_f64(&size, "height");
        let (w, h) = if landscape {
            (portrait_h, portrait_w)
        } else {
            (portrait_w, portrait_h)
        };
        size.insert("width", w)?;
        size.insert("height", h)?;
    }
    Ok(())
}

/// Section 0's column count (`1` when there is no columns map). Lets the Layout
/// tab highlight the active column preset.
#[must_use]
pub fn document_column_count(loro: &LoroDoc) -> u8 {
    let sections = loro.get_list(KEY_SECTIONS);
    sections
        .get(0)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|section| child_map(&section, KEY_LAYOUT))
        .and_then(|layout| child_map(&layout, KEY_COLUMNS))
        .and_then(|cols| cols.get(KEY_COL_COUNT))
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_i64().ok())
        .map_or(1, |c| c.clamp(1, u8::MAX as i64) as u8)
}

/// Sets every section's column `count` (clamped to ≥ 1). A newly-created columns
/// map gets a default gap and no separator; an existing one keeps its gap and
/// separator — only the count changes. A `count` of 1 leaves a single-column
/// layout (the whole content width).
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn set_document_columns(loro: &LoroDoc, count: u8) -> Result<(), MutationError> {
    let count = count.max(1);
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
        let cols = match child_map(&layout, KEY_COLUMNS) {
            Some(m) => m,
            None => {
                let m = layout.insert_container(KEY_COLUMNS, LoroMap::new())?;
                m.insert(KEY_COL_GAP, DEFAULT_COL_GAP_PT)?;
                m.insert(KEY_COL_SEPARATOR, false)?;
                m
            }
        };
        cols.insert(KEY_COL_COUNT, i64::from(count))?;
    }
    Ok(())
}
