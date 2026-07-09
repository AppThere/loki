// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The root document type.
//!
//! [`Document`] is the top-level container for all document content and
//! metadata. It corresponds to both ODF's
//! `<office:document>/<office:body>/<office:text>` and OOXML's
//! `w:document/w:body`.

use crate::io::source::DocumentSource;
use crate::layout::section::Section;
use crate::meta::core::DocumentMeta;
use crate::settings::DocumentSettings;
use crate::style::catalog::{StyleCatalog, StyleId};
use crate::style::para_style::ParagraphStyle;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::ParaProps;
use loki_primitives::units::Points;

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
///     settings: None,
///     comments: Vec::new(),
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

    /// Document-wide settings (default tab stop, etc.).
    ///
    /// `None` means all settings use their format-defined defaults.
    /// OOXML: `word/settings.xml`; ODF: `settings.xml`.
    pub settings: Option<DocumentSettings>,

    /// Document comments (annotations), keyed by id. The content flow carries
    /// only [`crate::content::annotation::CommentRef`] anchors; the comment
    /// bodies live here. OOXML: `word/comments.xml`; ODF: `office:annotation`.
    pub comments: Vec<crate::content::annotation::Comment>,

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
            settings: None,
            comments: Vec::new(),
            source: None,
        }
    }

    /// Creates a blank document ready for editing.
    ///
    /// Contains one section with one empty paragraph so the cursor can be
    /// placed and text can be typed immediately.  The style catalog is
    /// pre-populated with built-in heading styles (H1–H6) so that applying
    /// a heading style from the style picker immediately produces a visible
    /// change in the rendered output.
    ///
    /// The page size is chosen based on the system locale: US Letter for
    /// `_US`, `_CA`, `_MX`, and other Letter-paper regions; A4 everywhere else.
    #[must_use]
    pub fn new_blank() -> Self {
        use crate::content::block::Block;
        use crate::layout::page::PageLayout;
        let layout = PageLayout {
            page_size: default_page_size_for_locale(),
            ..PageLayout::default()
        };
        let section = Section::with_layout_and_blocks(layout, vec![Block::Para(vec![])]);
        let mut styles = StyleCatalog::default();
        let heading_defs: &[(&str, &str, f32, bool)] = &[
            ("Heading1", "Heading 1", 24.0, true),
            ("Heading2", "Heading 2", 18.0, true),
            ("Heading3", "Heading 3", 14.0, true),
            ("Heading4", "Heading 4", 12.0, true),
            ("Heading5", "Heading 5", 10.0, true),
            ("Heading6", "Heading 6", 10.0, false),
        ];
        for &(id, name, size_pt, bold) in heading_defs {
            let style = ParagraphStyle {
                id: StyleId::new(id),
                display_name: Some(name.to_string()),
                parent: None,
                linked_char_style: None,
                next_style_id: None,
                para_props: ParaProps::default(),
                char_props: CharProps {
                    bold: Some(bold),
                    font_size: Some(Points::new(f64::from(size_pt))),
                    ..Default::default()
                },
                is_default: false,
                is_custom: false,
                extensions: Default::default(),
            };
            styles.paragraph_styles.insert(StyleId::new(id), style);
        }
        Self {
            meta: DocumentMeta::default(),
            styles,
            sections: vec![section],
            settings: None,
            comments: Vec::new(),
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
    pub fn flat_index_to_section_block(&self, flat_index: usize) -> Option<(usize, usize)> {
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

// ── Locale-based page-size helpers ────────────────────────────────────────────

/// Returns the appropriate default page size for the running locale.
///
/// Reads `LC_PAPER`, `LC_ALL`, `LANGUAGE`, and `LANG` environment variables
/// (in priority order).  Returns US Letter for regions that use it; A4 for
/// all others.  Falls back to A4 if no locale can be determined.
fn default_page_size_for_locale() -> crate::layout::page::PageSize {
    use crate::layout::page::PageSize;
    for var in &["LC_PAPER", "LC_ALL", "LANGUAGE", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let upper = val.to_uppercase();
            if upper.is_empty() {
                continue;
            }
            if uses_letter_paper(&upper) {
                return PageSize::letter();
            }
            return PageSize::a4();
        }
    }
    PageSize::a4()
}

/// Returns `true` when `locale_upper` (an uppercased locale string) indicates
/// a region that uses US Letter paper by convention.
fn uses_letter_paper(locale_upper: &str) -> bool {
    const LETTER_REGIONS: &[&str] = &[
        "_US", "_CA", "_MX", "_PH", "_CO", "_CL", "_VE", "_BO", "_SV", "_GT", "_HN", "_NI", "_CR",
        "_DO", "_PR",
    ];
    LETTER_REGIONS.iter().any(|r| locale_upper.contains(r))
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
