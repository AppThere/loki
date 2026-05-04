// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! The root document type.
//!
//! [`Document`] is the top-level container for all document content and
//! metadata. It corresponds to both ODF's
//! `<office:document>/<office:body>/<office:text>` and OOXML's
//! `w:document/w:body`.

use crate::io::source::DocumentSource;
use crate::layout::section::Section;
use crate::meta::core::DocumentMeta;
use crate::style::catalog::StyleCatalog;

/// The root of a Loki document.
///
/// A document is composed of metadata, a style catalog, a sequence of
/// sections (each containing blocks), and provenance information about
/// the source format. This structure maps to both ODF's
/// `<office:document>/<office:body>/<office:text>` and OOXML's
/// `w:document/w:body`.
///
/// # Examples
///
/// ```
/// use loki_doc_model::Document;
/// use loki_doc_model::meta::DocumentMeta;
/// use loki_doc_model::style::StyleCatalog;
/// use loki_doc_model::layout::Section;
/// use loki_doc_model::content::block::Block;
/// use loki_doc_model::content::inline::Inline;
/// use loki_doc_model::content::attr::NodeAttr;
///
/// let mut meta = DocumentMeta::default();
/// meta.title = Some("My Document".into());
///
/// let heading = Block::Heading(1, NodeAttr::default(), vec![
///     Inline::Str("Introduction".into()),
/// ]);
///
/// let mut section = Section::new();
/// section.blocks.push(heading);
///
/// let doc = Document {
///     meta,
///     styles: StyleCatalog::default(),
///     sections: vec![section],
///     source: None,
/// };
///
/// assert_eq!(doc.sections.len(), 1);
/// assert_eq!(doc.meta.title.as_deref(), Some("My Document"));
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Document {
    /// Document metadata (title, author, dates, language). §7.2.1 TR 29166.
    pub meta: DocumentMeta,

    /// The style catalog. All style references in content resolve here.
    pub styles: StyleCatalog,

    /// The document body as a sequence of sections.
    ///
    /// If the document has no explicit section breaks, there is exactly one
    /// section containing all content.
    pub sections: Vec<Section>,

    /// Format and version provenance from the source file, if loaded from one.
    ///
    /// `None` for programmatically constructed documents.
    pub source: Option<DocumentSource>,
}

impl Document {
    /// Creates a new empty document with a single default section.
    #[must_use]
    pub fn new() -> Self {
        Self {
            meta: DocumentMeta::default(),
            styles: StyleCatalog::default(),
            sections: vec![Section::new()],
            source: None,
        }
    }

    /// Returns a reference to the first section, if any.
    #[must_use]
    pub fn first_section(&self) -> Option<&Section> {
        self.sections.first()
    }

    /// Returns a mutable reference to the first section, if any.
    pub fn first_section_mut(&mut self) -> Option<&mut Section> {
        self.sections.first_mut()
    }

    /// Returns a slice of all sections in the document.
    ///
    /// Returns an empty slice for an empty document. Most documents have
    /// exactly one section; only documents with explicit section breaks
    /// (e.g. different page orientations) have more than one.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Returns a mutable slice of all sections in the document.
    ///
    /// Allows in-place mutation of any section's blocks or layout without
    /// replacing the section. Returns an empty slice for an empty document.
    pub fn sections_mut(&mut self) -> &mut [Section] {
        &mut self.sections
    }

    /// Returns the section at `index`, or `None` if `index` is out of range.
    ///
    /// Index `0` is the first (and most commonly the only) section.
    #[must_use]
    pub fn section_at(&self, index: usize) -> Option<&Section> {
        self.sections.get(index)
    }

    /// Returns a mutable reference to the section at `index`, or `None` if
    /// `index` is out of range.
    pub fn section_at_mut(&mut self, index: usize) -> Option<&mut Section> {
        self.sections.get_mut(index)
    }

    /// Returns the number of sections in the document.
    ///
    /// Returns `0` for a document whose `sections` field was explicitly
    /// cleared. [`Document::new`] always starts with one section.
    #[must_use]
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Returns an iterator over all blocks across all sections in document
    /// order.
    ///
    /// Blocks are yielded section by section, then in block order within each
    /// section. The position of a block in this iterator corresponds to its
    /// flat index as used by the Loro bridge (`block_0`, `block_1`, …).
    pub fn blocks_flat(&self) -> impl Iterator<Item = &crate::content::block::Block> {
        self.sections.iter().flat_map(|s| s.blocks.iter())
    }

    /// Returns the block at flat index `index` across all sections, or `None`
    /// if `index` is out of range.
    ///
    /// Flat indices are assigned by iterating sections in order, then blocks
    /// within each section. For example, in a document with two sections of
    /// two blocks each, flat index `2` is the first block of the second
    /// section.
    ///
    /// Flat indices are stable within a document snapshot but are **not**
    /// preserved across mutations that insert or remove blocks.
    #[must_use]
    pub fn block_at_flat(&self, index: usize) -> Option<&crate::content::block::Block> {
        self.blocks_flat().nth(index)
    }

    /// Returns the total number of blocks across all sections.
    ///
    /// Returns `0` for an empty document (no sections or all sections empty).
    #[must_use]
    pub fn block_count_flat(&self) -> usize {
        self.sections.iter().map(|s| s.blocks.len()).sum()
    }

    /// Returns the `(section_index, block_index_within_section)` pair for a
    /// given flat block index, or `None` if `flat_index` is out of range.
    ///
    /// Useful for locating which section owns a block when only its flat index
    /// is known (e.g. after receiving a Loro mutation targeting `block_N`).
    ///
    /// # Examples
    ///
    /// For a document with two sections of two blocks each:
    /// - `flat_index_to_section_block(0)` → `Some((0, 0))`
    /// - `flat_index_to_section_block(2)` → `Some((1, 0))`
    /// - `flat_index_to_section_block(4)` → `None`
    #[must_use]
    pub fn flat_index_to_section_block(
        &self,
        flat_index: usize,
    ) -> Option<(usize, usize)> {
        let mut remaining = flat_index;
        for (s_idx, section) in self.sections.iter().enumerate() {
            if remaining < section.blocks.len() {
                return Some((s_idx, remaining));
            }
            remaining -= section.blocks.len();
        }
        None
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::block::Block;
    use crate::layout::page::{PageLayout, PageSize};

    fn hr() -> Block {
        Block::HorizontalRule
    }

    fn make_doc_with_sections(blocks_per_section: &[usize]) -> Document {
        let mut doc = Document::new();
        doc.sections.clear();
        for &count in blocks_per_section {
            let blocks = (0..count).map(|_| hr()).collect();
            doc.sections.push(Section::with_layout_and_blocks(PageLayout::default(), blocks));
        }
        doc
    }

    #[test]
    fn document_new_has_one_section() {
        let doc = Document::new();
        assert_eq!(doc.sections.len(), 1);
        assert!(doc.meta.title.is_none());
        assert!(doc.source.is_none());
    }

    #[test]
    fn document_two_sections_different_sizes() {
        let mut doc = Document::new();

        let mut layout2 = PageLayout::default();
        layout2.page_size = PageSize::a4();
        let section2 = Section::with_layout_and_blocks(layout2, vec![]);
        doc.sections.push(section2);

        assert_eq!(doc.sections.len(), 2);
        assert_ne!(
            doc.sections[0].layout.page_size,
            doc.sections[1].layout.page_size,
        );
    }

    // ── section_count / section_at ────────────────────────────────────────────

    #[test]
    fn section_count_empty_document() {
        let doc = make_doc_with_sections(&[]);
        assert_eq!(doc.section_count(), 0);
        assert!(doc.section_at(0).is_none());
    }

    #[test]
    fn section_at_single_section() {
        let doc = make_doc_with_sections(&[2]);
        assert_eq!(doc.sections().len(), 1);
        assert!(doc.section_at(0).is_some());
        assert!(doc.section_at(1).is_none());
    }

    #[test]
    fn sections_mut_allows_modification() {
        let mut doc = make_doc_with_sections(&[1]);
        doc.sections_mut()[0].blocks.push(hr());
        assert_eq!(doc.section_at(0).unwrap().blocks.len(), 2);
    }

    // ── block_count_flat / block_at_flat ──────────────────────────────────────

    #[test]
    fn block_count_flat_empty_document() {
        let doc = make_doc_with_sections(&[]);
        assert_eq!(doc.block_count_flat(), 0);
        assert!(doc.block_at_flat(0).is_none());
    }

    #[test]
    fn block_at_flat_single_section_three_blocks() {
        let doc = make_doc_with_sections(&[3]);
        assert_eq!(doc.block_count_flat(), 3);
        assert!(doc.block_at_flat(2).is_some());
        assert!(doc.block_at_flat(3).is_none());
    }

    #[test]
    fn block_at_flat_two_sections_two_blocks_each() {
        let doc = make_doc_with_sections(&[2, 2]);
        assert_eq!(doc.block_count_flat(), 4);
        // flat index 2 is the first block of the second section
        assert!(doc.block_at_flat(2).is_some());
        assert!(doc.block_at_flat(4).is_none());
    }

    // ── flat_index_to_section_block ───────────────────────────────────────────

    #[test]
    fn flat_index_to_section_block_first_block() {
        let doc = make_doc_with_sections(&[2, 2]);
        assert_eq!(doc.flat_index_to_section_block(0), Some((0, 0)));
    }

    #[test]
    fn flat_index_to_section_block_crosses_section_boundary() {
        let doc = make_doc_with_sections(&[2, 2]);
        assert_eq!(doc.flat_index_to_section_block(2), Some((1, 0)));
    }

    #[test]
    fn flat_index_to_section_block_out_of_range() {
        let doc = make_doc_with_sections(&[2, 2]);
        assert!(doc.flat_index_to_section_block(4).is_none());
    }

    // ── blocks_flat ──────────────────────────────────────────────────────────

    #[test]
    fn blocks_flat_yields_all_blocks_in_order() {
        let doc = make_doc_with_sections(&[2, 3]);
        let count = doc.blocks_flat().count();
        assert_eq!(count, 5);
    }
}
