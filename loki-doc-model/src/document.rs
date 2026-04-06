// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::page::{PageLayout, PageSize};

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
}
