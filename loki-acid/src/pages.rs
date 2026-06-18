// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Pagination and glyph-coverage analysis — the rasteriser-free canaries.
//!
//! The plan names **page-count drift** and **glyph coverage (no tofu /
//! `.notdef`)** as the cheap canaries to assert before any per-pixel diff.
//! Both are computed here directly from the layout, with no GPU.

use loki_layout::{
    DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout, PositionedItem,
    layout_document,
};
use serde::{Deserialize, Serialize};

use loki_doc_model::Document;

/// Lays `doc` out in paginated mode (the print-fidelity geometry the references
/// are produced from).
#[must_use]
pub fn paginate(doc: &Document) -> PaginatedLayout {
    let mut resources = FontResources::new();
    match layout_document(
        &mut resources,
        doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    ) {
        DocumentLayout::Paginated(p) => p,
        // Paginated mode always yields a paginated layout.
        _ => PaginatedLayout {
            page_size: Default::default(),
            pages: Vec::new(),
        },
    }
}

/// Glyph-coverage summary for a laid-out document.
///
/// A `.notdef` glyph (id 0) is "tofu": a character the resolved font could not
/// render. Any tofu is a P0 fidelity failure (garbled glyphs).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlyphCoverage {
    /// Total glyphs across every page.
    pub total_glyphs: usize,
    /// Glyphs rendered as `.notdef` (id 0).
    pub notdef_glyphs: usize,
    /// 0-based page indices that contain at least one tofu glyph.
    pub pages_with_tofu: Vec<usize>,
}

impl GlyphCoverage {
    /// Fraction of glyphs that resolved to a real glyph, in `[0.0, 1.0]`.
    /// Returns `1.0` for a document with no glyphs.
    #[must_use]
    pub fn coverage_ratio(&self) -> f64 {
        if self.total_glyphs == 0 {
            return 1.0;
        }
        let resolved = self.total_glyphs - self.notdef_glyphs;
        resolved as f64 / self.total_glyphs as f64
    }

    /// `true` when any tofu glyph was found.
    #[must_use]
    pub fn has_tofu(&self) -> bool {
        self.notdef_glyphs > 0
    }
}

/// Scans a paginated layout and reports glyph coverage.
#[must_use]
pub fn glyph_coverage(layout: &PaginatedLayout) -> GlyphCoverage {
    let mut cov = GlyphCoverage::default();
    for (page_idx, page) in layout.pages.iter().enumerate() {
        let mut page_has_tofu = false;
        for item in page.all_items() {
            scan_item(item, &mut cov, &mut page_has_tofu);
        }
        if page_has_tofu {
            cov.pages_with_tofu.push(page_idx);
        }
    }
    cov
}

fn scan_item(item: &PositionedItem, cov: &mut GlyphCoverage, page_has_tofu: &mut bool) {
    match item {
        PositionedItem::GlyphRun(run) => {
            for glyph in &run.glyphs {
                cov.total_glyphs += 1;
                if glyph.id == 0 {
                    cov.notdef_glyphs += 1;
                    *page_has_tofu = true;
                }
            }
        }
        PositionedItem::ClippedGroup { items, .. } | PositionedItem::RotatedGroup { items, .. } => {
            for child in items {
                scan_item(child, cov, page_has_tofu);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_ratio_handles_empty() {
        let cov = GlyphCoverage::default();
        assert_eq!(cov.coverage_ratio(), 1.0);
        assert!(!cov.has_tofu());
    }

    #[test]
    fn coverage_ratio_with_tofu() {
        let cov = GlyphCoverage {
            total_glyphs: 100,
            notdef_glyphs: 5,
            pages_with_tofu: vec![2],
        };
        assert!((cov.coverage_ratio() - 0.95).abs() < 1e-9);
        assert!(cov.has_tofu());
    }
}
