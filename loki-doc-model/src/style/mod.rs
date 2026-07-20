// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Named style definitions and the style catalog.
//!
//! Both ODF and OOXML maintain a catalog of named styles. This module
//! provides the abstract representation. TR 29166 §7.2.3.
//!
//! See ADR-0007 for the rationale behind using `IndexMap` in the catalog.

pub mod catalog;
pub mod char_style;
pub mod list_style;
pub mod page_style;
pub mod para_style;
pub mod props;
pub mod resolve;
pub mod resolve_table;
pub mod table_banding;
pub mod table_borders;
pub mod table_cnf;
pub mod table_style;
pub mod tree;

pub use catalog::{ResolvedCharProps, ResolvedParaProps, StyleCatalog, StyleId};
pub use char_style::CharacterStyle;
pub use list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};
pub use page_style::{PageStyle, derive_page_styles, section_page_style_ids};
pub use para_style::ParagraphStyle;
pub use resolve::{Provenance, Resolved};
pub use table_banding::resolve_cell_shading;
pub use table_borders::{CellEdges, TableBorders};
pub use table_cnf::TableCnf;
pub use table_style::{TableConditionalFormat, TableLook, TableRegion, TableStyle};
