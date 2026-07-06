// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Named page style definition (Spec 05 M6 page family, ADR-0012 Decision 2).
//!
//! Loki adopts the **ODF model** as the unified representation: page styling is a
//! named, catalogued family carrying the page geometry (size, margins,
//! orientation, columns) and its header/footer master — everything a
//! [`PageLayout`] already holds. Page styles are the family's **explicit
//! exception to the inheritance tree**: neither OOXML nor ODF gives page styles a
//! `basedOn` parent, so a [`PageStyle`] has **no parent** and the tree view
//! degrades to a flat list (like the list family). Resolution therefore needs no
//! page-specific code — a non-inheriting family is a chain of length one, so the
//! inspector shows only `Local` (set on this page style) and `FormatDefault`.
//!
//! On export the mapping inverts the import:
//! - **ODT** writes each page style natively as `style:page-layout` +
//!   `style:master-page`.
//! - **DOCX** has no named page style, so each page style maps to the section
//!   properties (`w:sectPr`) of the sections that use it.

use indexmap::IndexMap;

use crate::content::attr::ExtensionBag;
use crate::layout::page::PageLayout;
use crate::layout::section::Section;
use crate::style::catalog::StyleId;

/// A named page style: page geometry + header/footer master, keyed in the
/// catalog's `page_styles`. **Non-inheriting** (no `parent`) — see the module
/// docs and ADR-0012 Decision 2.
///
/// ODF: `style:page-layout` + `style:master-page`.
/// OOXML: the section properties (`w:sectPr`) of the sections that use it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageStyle {
    /// The unique identifier used to reference this style. In ODF this is the
    /// master-page name; in OOXML it is a Loki-assigned name for the section
    /// geometry (OOXML has no named page style).
    pub id: StyleId,

    /// A human-readable display name shown in the UI.
    /// ODF `style:display-name`; no OOXML equivalent (falls back to `id`).
    pub display_name: Option<String>,

    /// The page geometry and header/footer master this style applies: size,
    /// margins, orientation, columns, headers/footers, and page numbering.
    pub layout: PageLayout,

    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

impl PageStyle {
    /// Creates a page style with the given id and layout, no display name.
    #[must_use]
    pub fn new(id: StyleId, layout: PageLayout) -> Self {
        Self {
            id,
            display_name: None,
            layout,
            extensions: ExtensionBag::default(),
        }
    }
}

/// Derives the catalog's page styles from a document's `sections` (ADR-0012
/// Decision 2's import mapping, format-neutral). Sections sharing an identical
/// [`PageLayout`] collapse to one page style, named `PageStyleN` in first-seen
/// order — OOXML has no page-style name to carry, and a deduped catalog is the
/// named representation the page panel and the DOCX section-export inverse both
/// need. The returned map is keyed by the assigned id; use
/// [`section_page_style_ids`] for the per-section id list (the export inverse).
#[must_use]
pub fn derive_page_styles(sections: &[Section]) -> IndexMap<StyleId, PageStyle> {
    let mut out: IndexMap<StyleId, PageStyle> = IndexMap::new();
    for section in sections {
        if out.values().any(|ps| ps.layout == section.layout) {
            continue; // an identical layout already has a page style
        }
        let id = StyleId::new(format!("PageStyle{}", out.len() + 1));
        out.insert(id.clone(), PageStyle::new(id, section.layout.clone()));
    }
    out
}

/// The page-style id each section maps to, in section order — the inverse of
/// [`derive_page_styles`] (DOCX export writes each id's geometry as the
/// section's `w:sectPr`). Sections with an identical layout share an id.
#[must_use]
pub fn section_page_style_ids(sections: &[Section]) -> Vec<StyleId> {
    let styles = derive_page_styles(sections);
    sections
        .iter()
        .map(|section| {
            styles
                .iter()
                .find(|(_, ps)| ps.layout == section.layout)
                .map(|(id, _)| id.clone())
                // Unreachable: derive_page_styles inserts one per distinct layout.
                .unwrap_or_else(|| StyleId::new("PageStyle1"))
        })
        .collect()
}

#[cfg(test)]
#[path = "page_style_tests.rs"]
mod tests;
