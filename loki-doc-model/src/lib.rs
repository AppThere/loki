// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Format-neutral document model for the Loki suite (text documents).
//!
//! `loki-doc-model` defines the abstract document model that `loki-odf` and
//! `loki-ooxml` both import from and export to. It knows nothing about ZIP,
//! XML, ODF namespaces, or OOXML namespaces — it is a pure Rust data model.
//!
//! ## Design Authority
//!
//! The design is grounded in two reference documents:
//!
//! - **Pandoc AST (`Text.Pandoc.Definition`)** — The content layer's
//!   `Block`/`Inline` hierarchy is adopted directly from pandoc, which has
//!   been validated against 30+ formats over 18 years. See ADR-0001.
//!
//! - **ISO/IEC TR 29166:2011** — The technical report on ODF/OOXML
//!   translation. Its §6.2 feature analysis tables define
//!   [`style::props::CharProps`] and [`style::props::ParaProps`].
//!   Its §8 complexity classifications determine what gets a
//!   first-class model type vs. [`content::attr::ExtensionBag`] storage.
//!   See ADR-0002.
//!
//! ## Architecture
//!
//! ```text
//! Document
//! ├── meta/    ← DocumentMeta: title, author, dates, language, custom props
//! ├── style/   ← StyleCatalog: named para/char/table/list style definitions
//! ├── content/ ← Block and Inline trees (the document body)
//! │   ├── block/
//! │   ├── inline/
//! │   ├── table/
//! │   └── field/
//! └── layout/  ← PageLayout, Section boundaries, headers/footers
//! ```
//!
//! ## Usage Example
//!
//! ```rust
//! use loki_doc_model::Document;
//! use loki_doc_model::meta::DocumentMeta;
//! use loki_doc_model::style::{StyleCatalog, StyleId, ParagraphStyle};
//! use loki_doc_model::style::props::{ParaProps, CharProps};
//! use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
//! use loki_doc_model::content::block::{Block, StyledParagraph};
//! use loki_doc_model::content::inline::Inline;
//! use loki_doc_model::content::table::{
//!     Table, TableHead, TableBody, TableFoot, TableCaption, ColSpec, Row, Cell,
//! };
//! use loki_doc_model::layout::Section;
//!
//! // 1. Metadata
//! let mut meta = DocumentMeta::default();
//! meta.title = Some("Project Report".into());
//! meta.creator = Some("Alice".into());
//!
//! // 2. Style catalog with a "Heading1" style
//! let mut styles = StyleCatalog::default();
//! let heading_style = ParagraphStyle {
//!     id: StyleId::new("Heading1"),
//!     display_name: Some("Heading 1".into()),
//!     parent: None,
//!     linked_char_style: None,
//!     next_style_id: None,
//!     para_props: ParaProps::default(),
//!     char_props: CharProps {
//!         bold: Some(true),
//!         font_size: Some(loki_primitives::units::Points::new(20.0)),
//!         ..Default::default()
//!     },
//!     is_default: false,
//!     is_custom: false,
//!     extensions: ExtensionBag::default(),
//! };
//! styles.paragraph_styles.insert(StyleId::new("Heading1"), heading_style);
//!
//! // 3. Content: a heading, two styled paragraphs, and a 2×2 table
//! let heading = Block::Heading(
//!     1,
//!     NodeAttr::default(),
//!     vec![Inline::Str("Introduction".into())],
//! );
//!
//! let para1 = Block::StyledPara(StyledParagraph {
//!     style_id: Some(StyleId::new("Normal")),
//!     direct_para_props: None,
//!     direct_char_props: None,
//!     inlines: vec![Inline::Str("First paragraph.".into())],
//!     attr: NodeAttr::default(),
//! });
//!
//! let para2 = Block::StyledPara(StyledParagraph {
//!     style_id: Some(StyleId::new("Normal")),
//!     direct_para_props: None,
//!     direct_char_props: None,
//!     inlines: vec![Inline::Str("Second paragraph.".into())],
//!     attr: NodeAttr::default(),
//! });
//!
//! let table = Block::Table(Box::new(Table {
//!     attr: NodeAttr::default(),
//!     caption: TableCaption::default(),
//!     width: None,
//!     col_specs: vec![
//!         ColSpec::proportional(1.0),
//!         ColSpec::proportional(1.0),
//!     ],
//!     head: TableHead::empty(),
//!     bodies: vec![TableBody::from_rows(vec![
//!         Row::new(vec![
//!             Cell::simple(vec![Block::Para(vec![Inline::Str("R1C1".into())])]),
//!             Cell::simple(vec![Block::Para(vec![Inline::Str("R1C2".into())])]),
//!         ]),
//!         Row::new(vec![
//!             Cell::simple(vec![Block::Para(vec![Inline::Str("R2C1".into())])]),
//!             Cell::simple(vec![Block::Para(vec![Inline::Str("R2C2".into())])]),
//!         ]),
//!     ])],
//!     foot: TableFoot::empty(),
//! }));
//!
//! // 4. Assemble into a document
//! let mut section = Section::new();
//! section.blocks.extend([heading, para1, para2, table]);
//!
//! let doc = Document {
//!     meta,
//!     styles,
//!     sections: vec![section],
//!     settings: None,
//!     comments: Vec::new(),
//!     source: None,
//! };
//!
//! assert_eq!(doc.sections.len(), 1);
//! assert_eq!(doc.sections[0].blocks.len(), 4);
//! ```

#![forbid(unsafe_code)]

pub mod content;
pub mod document;
pub mod loro_schema;
pub mod settings;
pub use loro_schema::*;
pub mod loro_bridge;
pub use loro_bridge::{
    BridgeError, IncrementalReader, document_to_loro, document_track_changes, loro_to_document,
    set_track_changes,
};
pub mod loro_mutation;
pub use loro_mutation::{
    BlockPath, PathStep, delete_selection_at, delete_text_at, get_block_text_at, get_mark_at_path,
    insert_text_at, insert_text_tracked_at, mark_text_at, tracked_grapheme_delete,
};
pub use loro_mutation::{
    MutationError, accept_reject_all_revisions, clear_block_list, delete_block, delete_text,
    document_column_count, document_is_landscape, document_margins, document_page_size,
    get_block_alignment, get_block_alignment_at, get_block_list_id, get_block_style_name,
    get_block_text, get_mark_at, insert_text, mark_text, merge_block, merge_block_at,
    rename_page_style, replace_text, set_block_alignment, set_block_alignment_at, set_block_style,
    set_block_type_heading, set_block_type_para, set_document_columns, set_document_margins,
    set_document_orientation, set_document_page_size, set_page_style_geometry, split_block,
    split_block_at,
};
#[cfg(feature = "serde")]
pub use loro_mutation::{
    delete_table_column, delete_table_row, first_toc_block_index, insert_block_after,
    insert_inline_image, insert_inline_image_at, insert_inline_note_at, insert_table_column,
    insert_table_of_contents, insert_table_row, refresh_table_of_contents, table_grid_dims,
};
pub mod error;
pub mod io;
pub mod layout;
pub mod meta;
pub mod style;

pub use document::Document;
pub use error::LokiDocError;
pub use settings::DocumentSettings;

/// Re-export of `loki_primitives` for consumers who need measurement types.
pub use loki_primitives;

// Type aliases to make common imports more ergonomic
pub use content::attr::{ExtensionBag, ExtensionKey, NodeAttr};
pub use content::revision_ops::{
    DeleteAction, accept_revisions, delete_action, has_revisions, reject_revisions,
};
pub use content::{Block, Inline};
pub use layout::Section;
pub use meta::DocumentMeta;
pub use style::props::{RevisionKind, RevisionMark};
pub use style::{Provenance, Resolved, StyleCatalog, StyleId};

/// Derive-macro re-exports (serde support is feature-gated).
#[cfg(feature = "serde")]
pub use serde;
