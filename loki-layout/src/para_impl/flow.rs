// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Entry point for resolving, laying out, and placing a single paragraph block.

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::list_style::ListLevelKind;

use crate::geometry::LayoutRect;
use crate::items::{PositionedImage, PositionedItem};
use crate::para::{format_list_marker, layout_paragraph};
use crate::resolve::{emu_to_pt, flatten_paragraph, pts_to_f32, resolve_para_props};

use super::place::place_paragraph_layout;
use crate::flow::{FlowState, finish_page};

/// Resolve, lay out, and place a single paragraph block.
pub(crate) fn flow_paragraph(state: &mut FlowState, para: &StyledParagraph, block_index: usize) {
    let mut resolved = resolve_para_props(para, state.catalog);

    // ── List level indentation fallback ─────────────────────────────────────
    // OOXML defines indentation on both the paragraph and its numbering level.
    // The level's pPr is the authoritative indent when the paragraph's own
    // pPr carries no indent (both indent_start and indent_hanging are 0.0).
    // This handles documents where `w:ind` is only on the abstract num level.
    if let Some(ref lm) = resolved.list_marker
        && resolved.indent_start == 0.0
        && resolved.indent_hanging == 0.0
        && let Some(list_style) = state.catalog.list_styles.get(&lm.list_id)
        && let Some(level_def) = list_style.levels.get(lm.level as usize)
    {
        let level_indent = pts_to_f32(level_def.indent_start);
        let level_hanging = pts_to_f32(level_def.hanging_indent);
        if level_indent > 0.0 || level_hanging > 0.0 {
            resolved.indent_start = level_indent;
            resolved.indent_hanging = level_hanging;
        }
    }

    // ── List marker synthesis ────────────────────────────────────────────────
    // When the paragraph carries list membership, look up the list style,
    // advance the per-list counter, format the marker string, and prepend it
    // as an `Inline::Str` followed by a tab. Non-list paragraphs reset
    // `prev_list_id` so the next list starts fresh.
    let owned_para: Option<StyledParagraph> = if let Some(ref lm) = resolved.list_marker {
        if let Some(list_style) = state.catalog.list_styles.get(&lm.list_id) {
            if let Some(level_def) = list_style.levels.get(lm.level as usize) {
                let start_value = match &level_def.kind {
                    ListLevelKind::Numbered { start_value, .. } => *start_value,
                    _ => 1,
                };
                // New-list detection: a different list_id means a new list
                // is starting, so counters for this id are cleared.
                if state.prev_list_id.as_ref() != Some(&lm.list_id) {
                    state.list_counters.remove(&lm.list_id);
                }
                state.prev_list_id = Some(lm.list_id.clone());
                state.advance_counter(&lm.list_id, lm.level, start_value);
                let counters = state
                    .list_counters
                    .get(&lm.list_id)
                    .copied()
                    .unwrap_or([1u32; 9]);
                let marker_text = format_list_marker(&list_style.levels, lm.level, &counters);
                let mut cloned = para.clone();
                cloned
                    .inlines
                    .insert(0, Inline::Str(format!("{}\t", marker_text)));
                Some(cloned)
            } else {
                state.prev_list_id = None;
                None
            }
        } else {
            state.prev_list_id = None;
            None
        }
    } else {
        state.prev_list_id = None;
        None
    };
    let effective_para: &StyledParagraph = owned_para.as_ref().unwrap_or(para);
    // ────────────────────────────────────────────────────────────────────────

    let (text, spans, images, notes) =
        flatten_paragraph(effective_para, state.catalog, &mut state.note_counter);
    state.pending_footnotes.extend(notes);

    state.cursor_y += resolved.space_before;

    if resolved.page_break_before && state.mode.is_paginated() {
        finish_page(state);
    }

    let mut para_layout = layout_paragraph(
        state.resources,
        &text,
        &spans,
        &resolved,
        state.content_width,
        state.display_scale,
        state.options.preserve_for_editing,
    );

    // ── Inline image placement (gap #9) ──────────────────────────────────────
    // TODO(inline-image-flow): Parley has no inline image box support.
    // Images are prepended as a block-level prefix before paragraph text;
    // all existing items are shifted down to make room.
    let mut total_image_height = 0.0f32;
    let mut image_items: Vec<PositionedItem> = Vec::new();
    for img in &images {
        if img.cx_emu == 0 && img.cy_emu == 0 {
            continue; // zero-size image — skip without crashing
        }
        let w = emu_to_pt(img.cx_emu);
        let h = emu_to_pt(img.cy_emu);
        image_items.push(PositionedItem::Image(PositionedImage {
            rect: LayoutRect::new(0.0, total_image_height, w, h),
            src: img.src.clone(),
            alt: img.alt.clone(),
        }));
        total_image_height += h;
    }
    if total_image_height > 0.0 {
        // Expand background fill to cover image area (first item when present).
        if let Some(PositionedItem::FilledRect(bg)) = para_layout.items.first_mut() {
            bg.rect.size.height += total_image_height;
        }
        // Shift all existing paragraph items down by total image height.
        for item in &mut para_layout.items {
            item.translate(0.0, total_image_height);
        }
        para_layout.height += total_image_height;
        // Prepend image items (they render before paragraph text).
        image_items.append(&mut para_layout.items);
        para_layout.items = image_items;
    }

    place_paragraph_layout(state, &resolved, para_layout, block_index);

    if resolved.page_break_after && state.mode.is_paginated() {
        finish_page(state);
    }
}
