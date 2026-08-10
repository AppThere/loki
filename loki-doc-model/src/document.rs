// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The root document type.
//!
//! [`Document`] is the top-level container for all document content and
//! metadata. It corresponds to both ODF's
//! `<office:document>/<office:body>/<office:text>` and OOXML's
//! `w:document/w:body`.

#[path = "document_paper.rs"]
mod paper;

use crate::io::source::DocumentSource;
use crate::layout::section::Section;
use crate::meta::core::DocumentMeta;
use crate::settings::DocumentSettings;
use crate::style::catalog::StyleCatalog;
use crate::style::para_style::ParagraphStyle;

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
    /// Whether this document mirrors its margins on even (verso) pages.
    ///
    /// # One question, two places the formats put the answer
    ///
    /// ODF states it per page layout (`style:page-usage="mirrored"`); OOXML has
    /// only the document-wide `w:mirrorMargins` in `settings.xml`. Both reach
    /// the model — the first as [`crate::layout::page::PageUsage`] on each
    /// section's layout, the second as [`crate::settings::DocumentSettings`] —
    /// so "is this document mirrored" has two possible homes and every consumer
    /// that asked it directly was picking one.
    ///
    /// A union rather than a precedence, because the two never contradict: a
    /// DOCX import sets both, an ODT import sets only the layouts, and a
    /// programmatically-built document may set only the setting. Asking either
    /// alone silently drops one of those three origins — which is how an ODT
    /// with mirrored margins rendered single-sided (Spec 08 T6.1).
    #[must_use]
    pub fn mirrors_margins(&self) -> bool {
        self.settings.as_ref().is_some_and(|s| s.mirror_margins)
            || self
                .sections
                .iter()
                .any(|sec| sec.layout.page_usage.mirrors_margins())
    }
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
            page_size: paper::default_page_size_for_locale(),
            ..PageLayout::default()
        };
        let section = Section::with_layout_and_blocks(layout, vec![Block::Para(vec![])]);
        let mut styles = StyleCatalog::default();
        for level in 1..=6u8 {
            let style = ParagraphStyle::builtin_heading(level);
            styles.paragraph_styles.insert(style.id.clone(), style);
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
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

// ── Locale-based page-size helpers ────────────────────────────────────────────

/// Returns the appropriate default page size for the running locale.
///
#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
