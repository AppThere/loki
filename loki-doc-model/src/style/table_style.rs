// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Named table style definition.
//!
//! TR 29166 §6.2.4 and §7.2.4 describe the ODF and OOXML table models.
//! ODF uses `style:style style:family="table"`; OOXML uses
//! `w:style w:type="table"`.

use loki_primitives::units::Points;
use loki_primitives::color::DocumentColor;
use crate::content::attr::ExtensionBag;
use crate::style::catalog::StyleId;
use crate::style::props::border::Border;

/// Table width specification.
///
/// TR 29166 §6.2.4. ODF `style:width` or `style:rel-width`;
/// OOXML `w:tblW`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TableWidth {
    /// An absolute width in points.
    Absolute(Points),
    /// A percentage of the page text area width.
    Percent(f32),
    /// The table width is determined by its content (auto).
    Auto,
}

/// Horizontal alignment of a table on the page.
///
/// ODF `table:align`; OOXML `w:jc` on `w:tblPr`.
/// TR 29166 §6.2.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TableAlignment {
    /// Table is left-aligned (the default).
    #[default]
    Left,
    /// Table is centered.
    Center,
    /// Table is right-aligned.
    Right,
}

/// Table-level formatting properties.
///
/// TR 29166 §6.2.4 "Table formatting" and §7.2.4.
/// ODF: `style:table-properties`. OOXML: `w:tblPr`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableProps {
    /// The table width. `None` inherits from the style default.
    pub width: Option<TableWidth>,
    /// Horizontal alignment of the table block on the page.
    pub alignment: Option<TableAlignment>,
    /// Default cell padding (inside border) in points.
    pub cell_padding: Option<Points>,
    /// Cell spacing (border collapse separation). `None` = collapsed.
    pub cell_spacing: Option<Points>,
    /// Default outside border for all table edges.
    pub border: Option<Border>,
    /// Background color of the table.
    pub background_color: Option<DocumentColor>,
}

/// A named table style.
///
/// TR 29166 §7.2.4 (Table XML structure comparison).
/// ODF: `style:style style:family="table"`.
/// OOXML: `w:style w:type="table"`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableStyle {
    /// The unique identifier used to reference this style.
    pub id: StyleId,

    /// A human-readable display name.
    pub display_name: Option<String>,

    /// The parent style identifier.
    pub parent: Option<StyleId>,

    /// Table-level formatting properties.
    pub table_props: TableProps,

    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_style_default_props() {
        let style = TableStyle {
            id: StyleId("TableGrid".into()),
            display_name: Some("Table Grid".into()),
            parent: None,
            table_props: TableProps::default(),
            extensions: ExtensionBag::default(),
        };
        assert!(style.table_props.width.is_none());
        assert!(style.table_props.border.is_none());
    }
}
