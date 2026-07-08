// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Named `style:family="table"` style definitions for `styles.xml`, carrying
//! table-level geometry (width / alignment / background). These back the
//! `table:style-name` reference each `<table:table>` writes; region banding is
//! not a table-level ODF concept — it is baked per cell (see `tables.rs`).

use loki_doc_model::StyleCatalog;
use loki_doc_model::style::table_style::{TableAlignment, TableProps, TableStyle, TableWidth};

use super::xml::{attr, pt};

/// Emits a `<style:style style:family="table">` for each catalog table style
/// (skipping `__`-prefixed synthetics) into the current `<office:styles>`.
pub(super) fn write_table_styles(out: &mut String, catalog: &StyleCatalog) {
    for (id, style) in &catalog.table_styles {
        if id.as_str().starts_with("__") {
            continue;
        }
        emit_table_style(out, id.as_str(), style);
    }
}

fn emit_table_style(out: &mut String, name: &str, style: &TableStyle) {
    out.push_str("<style:style");
    attr(out, "style:name", name);
    if let Some(display) = &style.display_name {
        attr(out, "style:display-name", display);
    }
    attr(out, "style:family", "table");
    if let Some(parent) = &style.parent {
        attr(out, "style:parent-style-name", parent.as_str());
    }
    out.push('>');
    emit_table_properties(out, &style.table_props);
    out.push_str("</style:style>");
}

/// The `<style:table-properties/>` child, or nothing when no table-level
/// geometry is set.
fn emit_table_properties(out: &mut String, props: &TableProps) {
    let mut attrs = String::new();
    match props.width {
        Some(TableWidth::Absolute(w)) => attr(&mut attrs, "style:width", &pt(w)),
        Some(TableWidth::Percent(p)) => attr(&mut attrs, "style:rel-width", &format!("{p}%")),
        // Auto / None / future variants → renderer decides.
        _ => {}
    }
    if let Some(align) = props.alignment {
        attr(&mut attrs, "table:align", align_value(align));
    }
    if let Some(hex) = props
        .background_color
        .as_ref()
        .and_then(loki_primitives::color::DocumentColor::to_hex)
    {
        attr(&mut attrs, "fo:background-color", &hex);
    }
    if !attrs.is_empty() {
        out.push_str("<style:table-properties");
        out.push_str(&attrs);
        out.push_str("/>");
    }
}

/// ODF `table:align` value for a [`TableAlignment`].
fn align_value(align: TableAlignment) -> &'static str {
    match align {
        TableAlignment::Center => "center",
        TableAlignment::Right => "right",
        // Left + any future variant.
        _ => "left",
    }
}

#[cfg(test)]
#[path = "table_style_tests.rs"]
mod tests;
