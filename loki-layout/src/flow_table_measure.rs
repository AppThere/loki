// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Table layout helpers: column-width resolution and cell height measurement.

use std::collections::HashMap;

use loki_doc_model::content::block::Block;
use loki_doc_model::StyleCatalog;

use crate::LayoutOptions;
use crate::flow::FlowState;
use crate::flow_block::flow_block;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutSize};
use crate::items::PositionedItem;
use crate::mode::LayoutMode;
use crate::resolve::pts_to_f32;

// ── Max-x helper ─────────────────────────────────────────────────────────────

/// Returns the rightmost x coordinate reached by any item in `items`.
pub(crate) fn get_items_max_x(items: &[PositionedItem]) -> f32 {
    let mut max_x = 0.0f32;
    for item in items {
        let x = match item {
            PositionedItem::GlyphRun(r) => {
                let mut run_max = r.origin.x;
                for g in &r.glyphs {
                    let right = r.origin.x + g.x + g.advance;
                    if right > run_max {
                        run_max = right;
                    }
                }
                run_max
            }
            PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => {
                r.rect.origin.x + r.rect.size.width
            }
            PositionedItem::BorderRect(r) => r.rect.origin.x + r.rect.size.width,
            PositionedItem::Image(r) => r.rect.origin.x + r.rect.size.width,
            PositionedItem::Decoration(d) => d.x + d.width,
            PositionedItem::ClippedGroup { clip_rect, items } => {
                let inner_max = get_items_max_x(items);
                inner_max.min(clip_rect.origin.x + clip_rect.size.width)
            }
            PositionedItem::RotatedGroup {
                origin,
                content_width,
                ..
            } => origin.x + content_width,
        };
        if x > max_x {
            max_x = x;
        }
    }
    max_x
}

// ── Cell height measurement ───────────────────────────────────────────────────

/// Measure the height a cell would occupy given `cell_content_width`.
pub(crate) fn measure_cell_height(
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    cell: &loki_doc_model::content::table::row::Cell,
    cell_content_width: f32,
    idx: usize,
) -> f32 {
    use loki_doc_model::content::table::row::CellTextDirection;

    let pad_top = cell.props.padding_top.map(pts_to_f32).unwrap_or(0.0);
    let pad_bottom = cell.props.padding_bottom.map(pts_to_f32).unwrap_or(0.0);

    let is_rotated = matches!(
        cell.props.text_direction.as_ref(),
        Some(CellTextDirection::TbRl | CellTextDirection::TbLr | CellTextDirection::BtLr)
    );

    let flow_w = if is_rotated { 10000.0 } else { cell_content_width };

    let mut temp_state = FlowState {
        resources,
        catalog,
        mode: &LayoutMode::Pageless,
        display_scale,
        options,
        cursor_y: 0.0,
        content_width: flow_w,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::default(),
        margins: LayoutInsets::default(),
        page_content_height: 0.0,
        page_number: 1,
        warnings: Vec::new(),
        current_indent: 0.0,
        list_counters: HashMap::new(),
        prev_list_id: None,
        note_counter: 0,
        pending_footnotes: Vec::new(),
        current_paragraphs: Vec::new(),
    };

    for block in &cell.blocks {
        flow_block(&mut temp_state, block, idx);
    }

    if is_rotated {
        let max_x = get_items_max_x(&temp_state.current_items);
        max_x + pad_top + pad_bottom
    } else {
        temp_state.cursor_y + pad_top + pad_bottom
    }
}

// ── Column width resolution ───────────────────────────────────────────────────

/// Resolve per-column widths from the table spec against `state.content_width`.
pub(crate) fn resolve_column_widths(
    state: &FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
) -> Vec<f32> {
    use loki_doc_model::content::table::col::{ColWidth, TableWidth};

    let col_count = tbl.col_count().max(1);
    let table_width = match tbl.width.as_ref() {
        Some(TableWidth::Fixed(w)) => *w,
        Some(TableWidth::Percent(p)) => state.content_width * (p / 100.0),
        _ => state.content_width,
    };
    let table_width = table_width.max(0.0);

    let mut resolved_widths = vec![0.0f32; col_count];
    let mut proportional_shares = vec![0.0f32; col_count];
    let mut total_fixed_width = 0.0f32;
    let mut total_proportional_shares = 0.0f32;

    for i in 0..col_count {
        let spec = tbl.col_specs.get(i);
        let width_spec = spec.map(|s| s.width).unwrap_or(ColWidth::Default);
        match width_spec {
            ColWidth::Fixed(pts) => {
                let w = pts_to_f32(pts);
                resolved_widths[i] = w;
                total_fixed_width += w;
            }
            ColWidth::Proportional(share) => {
                proportional_shares[i] = share;
                total_proportional_shares += share;
            }
            ColWidth::Default | _ => {
                proportional_shares[i] = 1.0;
                total_proportional_shares += 1.0;
            }
        }
    }

    let remaining_width = (table_width - total_fixed_width).max(0.0);
    if total_proportional_shares > 0.0 {
        let share_unit = remaining_width / total_proportional_shares;
        for i in 0..col_count {
            let spec = tbl.col_specs.get(i);
            let width_spec = spec.map(|s| s.width).unwrap_or(ColWidth::Default);
            match width_spec {
                ColWidth::Proportional(_) | ColWidth::Default => {
                    resolved_widths[i] = proportional_shares[i] * share_unit;
                }
                _ => {}
            }
        }
    } else if total_fixed_width > 0.0 {
        let scale = table_width / total_fixed_width;
        for w in &mut resolved_widths {
            *w *= scale;
        }
    } else {
        let uniform_w = table_width / col_count as f32;
        resolved_widths.fill(uniform_w);
    }

    resolved_widths
}

// ── Cell block layout ─────────────────────────────────────────────────────────

/// Flow `blocks` in pageless mode and return the resulting positioned items.
///
/// Creates a temporary [`FlowState`] with the given geometry and flows all
/// `blocks` through [`flow_block`]. The resulting items are returned without
/// any additional translation; callers apply their own offsets.
#[allow(clippy::too_many_arguments)]
pub(crate) fn flow_cell_blocks(
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    blocks: &[Block],
    content_width: f32,
    starting_indent: f32,
    starting_y: f32,
    idx: usize,
) -> Vec<PositionedItem> {
    let mut temp_state = FlowState {
        resources,
        catalog,
        mode: &LayoutMode::Pageless,
        display_scale,
        options,
        cursor_y: starting_y,
        content_width,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::default(),
        margins: LayoutInsets::default(),
        page_content_height: 0.0,
        page_number: 1,
        warnings: Vec::new(),
        current_indent: starting_indent,
        list_counters: HashMap::new(),
        prev_list_id: None,
        note_counter: 0,
        pending_footnotes: Vec::new(),
        current_paragraphs: Vec::new(),
    };

    for block in blocks {
        flow_block(&mut temp_state, block, idx);
    }

    temp_state.current_items
}
