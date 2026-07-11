// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Resolves each section's ODT `style:master-page` / `style:page-layout` names,
//! honouring the stored [`Section::page_style`] reference so a named — or
//! renamed — page style round-trips through ODT export/import (ADR-0012
//! Decision 2's ODF-native mapping).
//!
//! Sections sharing a page style collapse to **one** master page (the first
//! referencing section's layout is the representative geometry — the same choice
//! the style panel makes), matching the `LibreOffice` model where a master page is
//! shared. A section without a stored reference falls back to the positional
//! [`master_page_name`], so documents built without page styles export exactly as
//! before.
//!
//! [`Section::page_style`]: loki_doc_model::layout::section::Section::page_style

use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;

use super::xml::{master_page_name, sanitize_ncname};

/// A distinct master page to emit in `styles.xml`.
pub(super) struct MasterPage {
    /// `style:master-page` name (also the `style:master-page-name` reference
    /// that `content.xml` attaches to a section's first paragraph).
    pub master: String,
    /// `style:display-name` — the page style's human-readable name, when the
    /// catalog gives it one distinct from the (sanitised) `master` name.
    pub display_name: Option<String>,
    /// The associated `style:page-layout` name.
    pub page_layout: String,
    /// The representative geometry + header/footer master.
    pub layout: PageLayout,
}

/// The resolved page-style naming for a whole document.
pub(super) struct PageStyleNames {
    /// The distinct master pages, in first-seen section order.
    pub masters: Vec<MasterPage>,
    /// Each section's `style:master-page` name (index-aligned with the sections;
    /// an empty document yields a single default entry).
    pub section_master: Vec<String>,
}

/// Resolve the master-page / page-layout names for every section of `doc`.
///
/// A section's stored `page_style` id becomes its master-page name (sanitised to
/// a valid XML `NCName`); a section without one keeps the positional
/// [`master_page_name`]. Distinct names are emitted once, so sections sharing a
/// page style share a master page.
#[must_use]
pub(super) fn resolve_page_style_names(doc: &Document) -> PageStyleNames {
    // Mirror `styles_xml`'s empty-document fallback: one default master page.
    if doc.sections.is_empty() {
        let master = master_page_name(0);
        return PageStyleNames {
            masters: vec![MasterPage {
                master: master.clone(),
                display_name: None,
                page_layout: "PL1".to_string(),
                layout: PageLayout::default(),
            }],
            section_master: vec![master],
        };
    }

    let mut masters: Vec<MasterPage> = Vec::new();
    let mut section_master: Vec<String> = Vec::with_capacity(doc.sections.len());
    for (idx, section) in doc.sections.iter().enumerate() {
        let name = section
            .page_style
            .as_ref()
            .map(|id| sanitize_ncname(id.as_str()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| master_page_name(idx));
        if !masters.iter().any(|m| m.master == name) {
            let page_layout = format!("PL{}", masters.len() + 1);
            // A distinct human name (only when it differs from the emitted
            // NCName) becomes `style:display-name`; the panel keeps the id.
            let display_name = section
                .page_style
                .as_ref()
                .and_then(|id| doc.styles.page_styles.get(id))
                .and_then(|ps| ps.display_name.clone())
                .filter(|dn| *dn != name);
            masters.push(MasterPage {
                master: name.clone(),
                display_name,
                page_layout,
                layout: section.layout.clone(),
            });
        }
        section_master.push(name);
    }
    PageStyleNames {
        masters,
        section_master,
    }
}

#[cfg(test)]
#[path = "page_styles_tests.rs"]
mod tests;
