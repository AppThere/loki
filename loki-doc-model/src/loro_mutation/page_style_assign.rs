// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Creating a named page style and **applying** it to a section (Spec 05 M6
//! page family, ADR-0012 Decision 2) — the two verbs that make the page family
//! a manager rather than an editor for whatever import happened to produce.
//!
//! [`super::page_style`] edits the geometry of a page style that already exists
//! and is already on some pages. These two add the other half of LibreOffice's
//! model: define a page style now, put it on a section later.
//!
//! # Why the pair is one unit
//!
//! Creating alone would be a decision with no reachable consumer. A page style
//! is visible on screen only through the sections that reference it, so a
//! catalog entry no section names paints nothing, exports nothing, and cannot
//! be told apart from a forgotten one. [`set_section_page_style`] is what makes
//! [`create_page_style`] mean something, and it is also the reader that turned
//! the catalog's geometry copy from a duplicate into a source — see
//! [`super::page_style`]'s module docs.

use loro::LoroDoc;

use super::MutationError;
use super::page_style::{section_at, sections_using, write_section_geometry};
use crate::layout::page::PageLayout;
use crate::loro_schema::{KEY_PAGE_STYLE_REF, KEY_SECTIONS};
use crate::style::catalog::StyleId;
use crate::style::page_style::PageStyle;

/// Creates the page style `name` with `layout` as its geometry, referenced by
/// no section yet. A no-op when `name` is empty or already names a page style
/// — never a silent overwrite, matching [`super::rename_page_style`]'s refusal
/// to clobber.
///
/// The new style is invisible in the document until
/// [`set_section_page_style`] puts it on a section; the panel lists it from the
/// catalog so it can be applied.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn create_page_style(
    loro: &LoroDoc,
    name: &str,
    layout: &PageLayout,
) -> Result<(), MutationError> {
    if name.is_empty() {
        return Ok(());
    }
    let id = StyleId::new(name);
    let mut catalog = crate::loro_bridge::read_document_styles(loro);
    if catalog.page_styles.contains_key(&id) {
        return Ok(());
    }
    catalog
        .page_styles
        .insert(id.clone(), PageStyle::new(id, layout.clone()));
    crate::loro_bridge::write_document_styles(loro, &catalog)
        .map_err(|e| MutationError::Loro(e.to_string()))
}

/// Points section `section_index` at the page style `name` **and gives it that
/// style's geometry**, so applying a page style changes the pages the section
/// occupies rather than only the label they carry.
///
/// The geometry comes from a section already using `name` when there is one —
/// the renderer's own source — and from the catalog entry otherwise, which is
/// the only copy an unapplied style has. A no-op when `name` is not a page
/// style or `section_index` is out of range.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn set_section_page_style(
    loro: &LoroDoc,
    section_index: usize,
    name: &str,
) -> Result<(), MutationError> {
    let catalog = crate::loro_bridge::read_document_styles(loro);
    let Some(style) = catalog.page_styles.get(&StyleId::new(name)) else {
        return Ok(());
    };
    let sections = loro.get_list(KEY_SECTIONS);
    let Some(target) = section_at(&sections, section_index) else {
        return Ok(());
    };

    // Prefer a live section's geometry over the catalog's copy: the two agree
    // once `set_page_style_geometry` has run, but a document imported before
    // that mutation existed carries a catalog entry it never wrote to.
    let layout: PageLayout = sections_using(loro, name)
        .into_iter()
        .find(|&s| s != section_index)
        .and_then(|s| section_at(&sections, s))
        .map(|m| crate::loro_bridge::reconstruct_page_layout(&m))
        .unwrap_or_else(|| style.layout.clone());

    target.insert(KEY_PAGE_STYLE_REF, name)?;
    write_section_geometry(&target, &layout)
}

#[cfg(test)]
#[path = "page_style_assign_tests.rs"]
mod tests;
