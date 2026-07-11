// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Writer for `w:type="table"` style definitions and a table instance's
//! `w:tblLook` — the export half of table-style banding (ECMA-376 §17.7.6,
//! §17.4.56).

use std::io::Write;

use quick_xml::Writer;

use loki_doc_model::StyleCatalog;
use loki_doc_model::style::table_style::{
    TableConditionalFormat, TableLook, TableProps, TableRegion, TableStyle,
};
use loki_primitives::color::DocumentColor;

use super::xml::{color_to_hex, write_empty, write_end, write_start, wval};

/// Emit every `w:type="table"` style definition in the catalog: band sizes,
/// base whole-table cell shading, and each `w:tblStylePr` conditional region.
pub(super) fn write_table_styles<W: Write>(w: &mut Writer<W>, catalog: &StyleCatalog) {
    for (id, style) in &catalog.table_styles {
        emit_table_style(w, id.as_str(), style, catalog);
    }
}

fn emit_table_style<W: Write>(
    w: &mut Writer<W>,
    sid: &str,
    style: &TableStyle,
    cat: &StyleCatalog,
) {
    let default_s = if cat.default_table_style.as_ref() == Some(&style.id) {
        "1"
    } else {
        "0"
    };
    let _ = write_start(
        w,
        "w:style",
        &[
            ("w:type", "table"),
            ("w:styleId", sid),
            ("w:default", default_s),
        ],
    );
    let name = style.display_name.as_deref().unwrap_or(sid);
    let _ = write_empty(w, "w:name", &wval(name));
    if let Some(parent) = &style.parent {
        let _ = write_empty(w, "w:basedOn", &wval(parent.as_str()));
    }
    write_tbl_pr(w, &style.table_props);
    if let Some(bg) = &style.table_props.background_color {
        write_cell_shd(w, bg);
    }
    for (region, fmt) in &style.conditional {
        emit_tbl_style_pr(w, *region, fmt);
    }
    let _ = write_end(w, "w:style");
}

/// `w:tblPr` carrying the band sizes; skipped entirely when neither is set.
fn write_tbl_pr<W: Write>(w: &mut Writer<W>, props: &TableProps) {
    if props.row_band_size.is_none() && props.col_band_size.is_none() {
        return;
    }
    let _ = write_start(w, "w:tblPr", &[]);
    if let Some(n) = props.row_band_size {
        let _ = write_empty(w, "w:tblStyleRowBandSize", &wval(&n.to_string()));
    }
    if let Some(n) = props.col_band_size {
        let _ = write_empty(w, "w:tblStyleColBandSize", &wval(&n.to_string()));
    }
    let _ = write_end(w, "w:tblPr");
}

/// A `<w:tcPr><w:shd .../></w:tcPr>` cell-shading wrapper.
fn write_cell_shd<W: Write>(w: &mut Writer<W>, color: &DocumentColor) {
    let _ = write_start(w, "w:tcPr", &[]);
    let hex = color_to_hex(color);
    let _ = write_empty(
        w,
        "w:shd",
        &[("w:val", "clear"), ("w:color", "auto"), ("w:fill", &hex)],
    );
    let _ = write_end(w, "w:tcPr");
}

fn emit_tbl_style_pr<W: Write>(
    w: &mut Writer<W>,
    region: TableRegion,
    fmt: &TableConditionalFormat,
) {
    let Some(name) = region_ooxml(region) else {
        return;
    };
    let _ = write_start(w, "w:tblStylePr", &[("w:type", name)]);
    if let Some(bg) = &fmt.background_color {
        write_cell_shd(w, bg);
    }
    let _ = write_end(w, "w:tblStylePr");
}

/// The OOXML `w:tblStylePr @w:type` name for a [`TableRegion`] (inverse of the
/// mapper's `map_table_region`); `None` for an unrecognised future variant.
fn region_ooxml(region: TableRegion) -> Option<&'static str> {
    Some(match region {
        TableRegion::WholeTable => "wholeTable",
        TableRegion::FirstRow => "firstRow",
        TableRegion::LastRow => "lastRow",
        TableRegion::FirstColumn => "firstCol",
        TableRegion::LastColumn => "lastCol",
        TableRegion::Band1Horz => "band1Horz",
        TableRegion::Band2Horz => "band2Horz",
        TableRegion::Band1Vert => "band1Vert",
        TableRegion::Band2Vert => "band2Vert",
        TableRegion::NwCell => "nwCell",
        TableRegion::NeCell => "neCell",
        TableRegion::SwCell => "swCell",
        TableRegion::SeCell => "seCell",
        _ => return None,
    })
}

/// Emit `w:tblLook` inside the current `w:tblPr` from a table instance's
/// encoded `"tbllook"` attr (absent or malformed ⇒ nothing written). Writes
/// both the explicit boolean attributes and the legacy `w:val` bitmask (both
/// of which the reader accepts).
pub(super) fn write_tbl_look<W: Write>(w: &mut Writer<W>, code: Option<&str>) {
    let Some(look) = code.and_then(TableLook::decode_attr) else {
        return;
    };
    let b = |on: bool| if on { "1" } else { "0" };
    let val = format!("{:04X}", look_bitmask(look));
    let _ = write_empty(
        w,
        "w:tblLook",
        &[
            ("w:val", &val),
            ("w:firstRow", b(look.first_row)),
            ("w:lastRow", b(look.last_row)),
            ("w:firstColumn", b(look.first_column)),
            ("w:lastColumn", b(look.last_column)),
            ("w:noHBand", b(!look.horizontal_banding)),
            ("w:noVBand", b(!look.vertical_banding)),
        ],
    );
}

/// The OOXML `w:tblLook @w:val` bitmask for a look (ECMA-376 §17.4.56).
fn look_bitmask(l: TableLook) -> u32 {
    let mut v = 0;
    if l.first_row {
        v |= 0x0020;
    }
    if l.last_row {
        v |= 0x0040;
    }
    if l.first_column {
        v |= 0x0080;
    }
    if l.last_column {
        v |= 0x0100;
    }
    if !l.horizontal_banding {
        v |= 0x0200;
    }
    if !l.vertical_banding {
        v |= 0x0400;
    }
    v
}

#[cfg(test)]
#[path = "table_style_tests.rs"]
mod tests;
