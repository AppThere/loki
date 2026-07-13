// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `style:table-properties` parsing and the bundled per-style property set
//! returned by `parse_style_props` (split from `styles.rs` to hold the
//! 300-line ceiling).

use quick_xml::events::BytesStart;

use crate::odt::model::styles::{OdfCellProps, OdfGraphicWrap, OdfParaProps, OdfTextProps};
use crate::odt::model::tables::OdfTableProps;
use crate::xml_util::local_attr_val;

/// Everything a `style:style` / `style:default-style` element's property
/// children can carry. One field per `style:*-properties` element kind.
#[derive(Debug, Default)]
pub(super) struct ParsedStyleProps {
    /// `style:paragraph-properties`.
    pub para_props: Option<OdfParaProps>,
    /// `style:text-properties`.
    pub text_props: Option<OdfTextProps>,
    /// `style:column-width` from `style:table-column-properties`.
    pub col_width: Option<String>,
    /// `style:table-cell-properties`.
    pub cell_props: Option<OdfCellProps>,
    /// `style:graphic-properties` wrap attributes.
    pub graphic_wrap: Option<OdfGraphicWrap>,
    /// `style:table-properties` (for `style:family="table"` styles).
    pub table_props: Option<OdfTableProps>,
}

/// Reads the attributes of a `style:table-properties` element.
pub(super) fn parse_table_props_element(e: &BytesStart<'_>) -> OdfTableProps {
    OdfTableProps {
        width: local_attr_val(e, b"width"),
        rel_width: local_attr_val(e, b"rel-width"),
        align: local_attr_val(e, b"align"),
        background_color: local_attr_val(e, b"background-color"),
    }
}
