// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-**page-style** geometry mutation (Spec 05 M6 page family, ADR-0012).
//!
//! The document's `set_document_*` page mutations ([`super::page`]) apply to
//! *every* section — the "change the whole document" model. This applies a page
//! layout to a **specific named page style** instead: the sections that
//! reference it (LibreOffice's model — editing a page style changes only the
//! pages that use it).
//!
//! It writes the whole geometry the page inspector shows — size, orientation,
//! margins (t/b/l/r), and columns — leaving header/footer/gutter distances and
//! page numbering untouched (the panel does not edit them).
//!
//! # Both copies of the geometry, or neither
//!
//! A page style's geometry exists twice: on every referencing [`Section`]'s
//! `layout` (what the renderer and both exporters read) and on the catalog's
//! [`PageStyle::layout`] (the definition an *unapplied* style has instead of a
//! section). [`set_page_style_geometry`] writes **both**, in one call, because
//! the moment [`set_section_page_style`] could seed a section from the catalog
//! copy, a stale catalog entry stopped being a harmless duplicate and became a
//! way to apply geometry no page has ever shown.
//!
//! [`Section`]: crate::layout::section::Section
//! [`PageStyle::layout`]: crate::style::page_style::PageStyle::layout
//! [`set_section_page_style`]: super::set_section_page_style

use loro::{LoroDoc, LoroList, LoroMap};

use super::MutationError;
use crate::layout::page::{PageLayout, PageOrientation};
use crate::loro_bridge::encode_col_widths;
use crate::loro_schema::{
    KEY_COL_COUNT, KEY_COL_GAP, KEY_COL_SEPARATOR, KEY_COL_WIDTHS, KEY_COLUMNS, KEY_LAYOUT,
    KEY_MARGIN_BOTTOM, KEY_MARGIN_LEFT, KEY_MARGIN_RIGHT, KEY_MARGIN_TOP, KEY_MARGINS,
    KEY_ORIENTATION, KEY_PAGE_SIZE, KEY_PAGE_STYLE_REF, KEY_SECTIONS,
};
use crate::style::catalog::StyleId;

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

/// The section map at `index`, or `None` when out of range or malformed.
pub(super) fn section_at(sections: &LoroList, index: usize) -> Option<LoroMap> {
    sections
        .get(index)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
}

/// The page-style reference string stored on a section map (`None` when unset).
pub(super) fn section_ref(section: &LoroMap) -> Option<String> {
    section
        .get(KEY_PAGE_STYLE_REF)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
}

/// The indices of every section whose stored page-style reference is `name`.
pub(super) fn sections_using(loro: &LoroDoc, name: &str) -> Vec<usize> {
    let sections = loro.get_list(KEY_SECTIONS);
    (0..sections.len())
        .filter(|&s| {
            section_at(&sections, s)
                .and_then(|m| section_ref(&m))
                .as_deref()
                == Some(name)
        })
        .collect()
}

/// Writes `layout`'s geometry — size, orientation, margins, columns — onto one
/// section map, leaving its headers/footers/gutter/page-numbering untouched.
pub(super) fn write_section_geometry(
    section: &LoroMap,
    layout: &PageLayout,
) -> Result<(), MutationError> {
    let lay = child_map_or_create(section, KEY_LAYOUT)?;

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

    // Columns: count (≥1), plus gap/separator/widths when a multi-column layout.
    // The widths key is written on *every* pass — deleted when the new layout has
    // none — because a preset that changes the column count leaves a width list
    // of the wrong length behind, and the reader would hand the layout engine
    // explicit widths the caller had already decided to drop.
    let cols = child_map_or_create(&lay, KEY_COLUMNS)?;
    let count = layout.columns.as_ref().map_or(1, |c| c.count).max(1);
    cols.insert(KEY_COL_COUNT, i64::from(count))?;
    // Gap and separator are **deleted** when the layout has no columns, for the
    // reason the widths key is rewritten every pass: dropping to one column
    // otherwise left the previous `separator: true` behind, the reader rebuilt
    // `Some(SectionColumns { count: 1, separator: true })`, and ODT export
    // wrote a `<style:column-sep>` inside a one-column `<style:columns>`.
    match layout.columns.as_ref() {
        Some(c) => {
            cols.insert(KEY_COL_GAP, c.gap.value())?;
            cols.insert(KEY_COL_SEPARATOR, c.separator)?;
        }
        None => {
            cols.delete(KEY_COL_GAP)?;
            cols.delete(KEY_COL_SEPARATOR)?;
        }
    }
    match layout.columns.as_ref().and_then(|c| {
        encode_col_widths(&c.widths).filter(|_| c.widths.len() == usize::from(c.count))
    }) {
        Some(joined) => cols.insert(KEY_COL_WIDTHS, joined)?,
        None => cols.delete(KEY_COL_WIDTHS)?,
    }
    Ok(())
}

/// Applies `layout`'s geometry to the page style `name`: every section that
/// references it, **and** the catalog entry that defines it (see the module
/// docs — both copies or neither). Sections that name a different page style,
/// and this style's headers/footers/gutter/page-numbering, are untouched.
///
/// A name no section references is still a valid target: an unapplied page
/// style is edited through its catalog entry alone.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn set_page_style_geometry(
    loro: &LoroDoc,
    name: &str,
    layout: &PageLayout,
) -> Result<(), MutationError> {
    let sections = loro.get_list(KEY_SECTIONS);
    for s in sections_using(loro, name) {
        let Some(section) = section_at(&sections, s) else {
            continue;
        };
        write_section_geometry(&section, layout)?;
    }

    // The catalog copy. An absent entry means nothing to update (a section may
    // carry a reference the catalog never got), which is not an error.
    let mut catalog = crate::loro_bridge::read_document_styles(loro);
    if let Some(ps) = catalog.page_styles.get_mut(&StyleId::new(name)) {
        ps.layout = layout.clone();
        crate::loro_bridge::write_document_styles(loro, &catalog)
            .map_err(|e| MutationError::Loro(e.to_string()))?;
    }
    Ok(())
}

/// Renames the page style `old` to `new`: updates the catalog entry (key + its
/// own id) and re-points every section that referenced `old`. A no-op when the
/// names are equal, `new` is empty or already taken (no silent merge), or `old`
/// is not a page style — so the caller can validate loosely.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn rename_page_style(loro: &LoroDoc, old: &str, new: &str) -> Result<(), MutationError> {
    if old == new || new.is_empty() {
        return Ok(());
    }
    let (old_id, new_id) = (StyleId::new(old), StyleId::new(new));
    let mut catalog = crate::loro_bridge::read_document_styles(loro);
    if catalog.page_styles.contains_key(&new_id) {
        return Ok(()); // don't clobber an existing style
    }
    let Some(mut ps) = catalog.page_styles.shift_remove(&old_id) else {
        return Ok(());
    };
    ps.id = new_id.clone();
    catalog.page_styles.insert(new_id, ps);
    crate::loro_bridge::write_document_styles(loro, &catalog)
        .map_err(|e| MutationError::Loro(e.to_string()))?;

    // Re-point every section that named the old page style.
    let sections = loro.get_list(KEY_SECTIONS);
    for s in 0..sections.len() {
        let Some(section) = section_at(&sections, s) else {
            continue;
        };
        if section_ref(&section).as_deref() == Some(old) {
            section.insert(KEY_PAGE_STYLE_REF, new)?;
        }
    }
    Ok(())
}
