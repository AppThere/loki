// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Named style definitions and the style catalog.
//!
//! Both ODF and OOXML maintain a catalog of named styles. This module
//! provides the abstract representation. TR 29166 §7.2.3.
//!
//! See ADR-0007 for the rationale behind using `IndexMap` in the catalog.

pub mod catalog;
pub mod char_style;
pub mod list_style;
pub mod para_style;
pub mod props;
pub mod table_style;

pub use catalog::{ResolvedCharProps, ResolvedParaProps, StyleCatalog, StyleId};
pub use char_style::CharacterStyle;
pub use list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};
pub use para_style::ParagraphStyle;
pub use table_style::TableStyle;
