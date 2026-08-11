// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Writing a [`PageLayout`] into its section's Loro map. Split from `mod.rs` at
//! the 300-line ceiling, on a real seam: this is the whole write side of page
//! geometry, where the parent is the document-level entry points.

use loro::{LoroMap, LoroMovableList};

use super::{BridgeError, decode, write::map_blocks_to_list};
use crate::layout::header_footer::HeaderFooter;
use crate::layout::page::{PageLayout, PageOrientation};
use crate::loro_schema::*;

pub(crate) fn map_page_layout(
    layout: &PageLayout,
    section_map: &LoroMap,
) -> Result<(), BridgeError> {
    let layout_map = section_map.insert_container(KEY_LAYOUT, LoroMap::new())?;

    let size_map = layout_map.insert_container(KEY_PAGE_SIZE, LoroMap::new())?;
    size_map.insert("width", layout.page_size.width.value())?;
    size_map.insert("height", layout.page_size.height.value())?;

    let margins_map = layout_map.insert_container(KEY_MARGINS, LoroMap::new())?;
    margins_map.insert(KEY_MARGIN_TOP, layout.margins.top.value())?;
    margins_map.insert(KEY_MARGIN_BOTTOM, layout.margins.bottom.value())?;
    margins_map.insert(KEY_MARGIN_LEFT, layout.margins.left.value())?;
    margins_map.insert(KEY_MARGIN_RIGHT, layout.margins.right.value())?;
    margins_map.insert(KEY_MARGIN_HEADER, layout.margins.header.value())?;
    margins_map.insert(KEY_MARGIN_FOOTER, layout.margins.footer.value())?;
    margins_map.insert(KEY_MARGIN_GUTTER, layout.margins.gutter.value())?;

    let orientation = match layout.orientation {
        PageOrientation::Portrait => "Portrait",
        PageOrientation::Landscape => "Landscape",
    };
    layout_map.insert(KEY_ORIENTATION, orientation)?;

    // Written through the ODF codec rather than a `Debug` string: the same
    // spelling the file format uses, so the CRDT and the document agree on one
    // vocabulary. Written unconditionally — a mark absent from the map is
    // indistinguishable from one an older snapshot never carried, and `All` is
    // the value a reader falls back to anyway.
    layout_map.insert(KEY_PAGE_USAGE, layout.page_usage.as_odf())?;

    if let Some(cols) = &layout.columns {
        let cols_map = layout_map.insert_container(KEY_COLUMNS, LoroMap::new())?;
        cols_map.insert(KEY_COL_COUNT, i64::from(cols.count))?;
        cols_map.insert(KEY_COL_GAP, cols.gap.value())?;
        cols_map.insert(KEY_COL_SEPARATOR, cols.separator)?;
        if let Some(joined) = decode::encode_col_widths(&cols.widths) {
            cols_map.insert(KEY_COL_WIDTHS, joined)?;
        }
    }

    map_header_footer_slot(&layout.header, KEY_HEADER, &layout_map)?;
    map_header_footer_slot(&layout.footer, KEY_FOOTER, &layout_map)?;
    map_header_footer_slot(&layout.header_first, KEY_HEADER_FIRST, &layout_map)?;
    map_header_footer_slot(&layout.footer_first, KEY_FOOTER_FIRST, &layout_map)?;
    map_header_footer_slot(&layout.header_even, KEY_HEADER_EVEN, &layout_map)?;
    map_header_footer_slot(&layout.footer_even, KEY_FOOTER_EVEN, &layout_map)?;

    Ok(())
}

pub(super) fn map_header_footer_slot(
    hf: &Option<HeaderFooter>,
    key: &str,
    layout_map: &LoroMap,
) -> Result<(), BridgeError> {
    if let Some(hf) = hf {
        let list = layout_map.insert_container(key, LoroMovableList::new())?;
        map_blocks_to_list(&hf.blocks, &list)?;
    }
    Ok(())
}
