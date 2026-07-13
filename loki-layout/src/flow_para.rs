// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph placement, splitting, and keep-with-next chain logic.
//!
//! Split algorithm (ADR 004 §3): a paragraph that does not fit is split at
//! the last fitting Parley line boundary; each fragment is wrapped in a
//! [`PositionedItem::ClippedGroup`] so full-height background/border items
//! clip correctly. `indent_hanging` shifts line 0 left for all paragraphs.
//! Known minor gap: `line_w` for `break_all_lines` is uniform across lines,
//! so line 0 wraps `indent_hanging` too early (no per-line width in Parley).
//!
//! Parley bidi note (gap #5): `BidiLevel`/`BidiResolver` are `pub(crate)` and
//! no `StyleProperty` sets a per-paragraph base direction — RTL direction is
//! deferred to a future Parley (workaround would be U+202B/U+200F controls).

use loki_doc_model::content::block::StyledParagraph;

use crate::geometry::LayoutRect;
use crate::items::{PositionedImage, PositionedItem};
use crate::para::{ParagraphLayout, ResolvedParaProps, layout_paragraph_spelled};
use crate::resolve::{emu_to_pt, pts_to_f32, resolve_para_props};

use super::columns_impl::break_column;
use super::editing::push_editing_para;
use super::{FlowState, LayoutWarning, finish_page};

#[path = "flow_para_chain.rs"]
mod chain;
#[path = "flow_para_place.rs"]
mod place;
#[path = "flow_split.rs"]
mod split;
#[path = "flow_widow_orphan.rs"]
mod widow_orphan;

pub(super) use chain::flow_keep_with_next_chain;
use place::place_paragraph_layout;
use split::split_and_place_loop;

// ── Public(super) API ─────────────────────────────────────────────────────────

/// Resolve, lay out, and place a single paragraph block.
pub(super) fn flow_paragraph(state: &mut FlowState, para: &StyledParagraph, block_index: usize) {
    let mut resolved = resolve_para_props(para, state.catalog);
    // Between-border group adjustment (gap #26), staged by the block loop.
    if let Some(ovr) = state.staged_between.take() {
        if ovr.suppress_top {
            resolved.border_top = None;
        }
        if let Some(bottom) = ovr.bottom {
            resolved.border_bottom = bottom;
        }
    }
    // Cell-content word-breaking: long unbreakable words wrap to the column.
    resolved.break_long_words = state.break_long_words;
    // Document default tab-stop interval (Word `w:defaultTabStop`), when set.
    if let Some(pt) = state.options.default_tab_stop_pt {
        resolved.default_tab_stop = pt;
    }

    // ── List level indentation fallback ─────────────────────────────────────
    // The numbering level's pPr is the authoritative indent when the paragraph
    // carries none (both indents 0.0) — e.g. `w:ind` only on the abstract num.
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
    // Prepend the label (bullet / number) as an `Inline::Str` + tab; a picture
    // bullet instead reports its image `src` for out-of-band placement below.
    let marker = super::flow_list_marker::synthesize(state, para, &resolved);
    let effective_para: &StyledParagraph = marker.owned.as_ref().unwrap_or(para);
    // ────────────────────────────────────────────────────────────────────────

    let (text, spans, mut images, mut notes) = crate::resolve::flatten_paragraph_with_base(
        effective_para,
        state.catalog,
        &mut state.note_counter,
        state.cell_char_defaults.as_ref(),
    );
    // Tag each note with its owning block + per-block order (see flow_footnotes).
    for (i, note) in notes.iter_mut().enumerate() {
        note.owner_block_index = block_index;
        note.note_in_block = i;
    }
    state.pending_footnotes.extend(notes);

    // ── Floating image wrap (gap #12): reserve a side band so text wraps
    // beside the float (emitted after text layout; removed from the
    // inline/block set so it is not also stacked above the text).
    let float_plan = super::float_impl::plan_float(&images, state.content_width);
    // Band geometry shared by this paragraph's own float (below) and the
    // `ActiveFloat` it may leave for following paragraphs.
    let own_float: Option<(f32, f32, bool)> = float_plan.as_ref().map(|(_, p)| {
        let inset = p.indent_start_delta + p.indent_end_delta;
        (inset, p.height, p.indent_start_delta > 0.0)
    });
    if let Some((idx, _)) = &float_plan
        && let Some((inset, height, shift_text)) = own_float
    {
        // The banded layout path narrows the lines beside the float and reflows
        // the rest at full width (one of the deltas is zero — left vs right).
        resolved.wrap_band = Some(crate::para::WrapBand {
            inset,
            cover_height: height,
            shift_text,
        });
        images.remove(*idx);
    }

    state.cursor_y += resolved.space_before;

    if resolved.page_break_before && state.mode.is_paginated() {
        finish_page(state);
    }

    // Cross-paragraph wrap: when this paragraph has no float of its own but an
    // earlier float still extends below the cursor, narrow it to clear the
    // remaining band (the part of the float still above the paragraph top).
    if own_float.is_none()
        && let Some(af) = &state.active_float
        && state.cursor_y < af.bottom_y - 0.5
    {
        resolved.wrap_band = Some(crate::para::WrapBand {
            inset: af.inset,
            cover_height: af.bottom_y - state.cursor_y,
            shift_text: af.shift_text,
        });
    }

    // Record comment start anchors at the paragraph's top (on the final page,
    // after any page break above) for the gutter comment panel.
    super::comments_impl::record_comment_anchors(state, &effective_para.inlines);

    let mut para_layout = layout_paragraph_spelled(
        state.resources,
        &text,
        &spans,
        &resolved,
        state.content_width,
        state.display_scale,
        state.options.preserve_for_editing,
        state.options.spell.as_ref(),
    );

    // Picture bullet (feature 5.4): place the label image in the hanging label
    // box on line 0. Injected into the paragraph's items so it translates with
    // the paragraph on placement.
    if let Some(src) = &marker.bullet_src
        && let Some(item) =
            super::flow_list_marker::picture_bullet_item(src, &resolved, &para_layout)
    {
        para_layout.items.push(item);
    }

    // ── Inline image placement (gap #9) ──────────────────────────────────────
    // TODO(inline-image-flow): no Parley inline image boxes; images are a
    // block-level prefix and existing items shift down to make room.
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

    // Emit the float beside the wrapped text; a float taller than its text
    // becomes an `ActiveFloat` so *following* paragraphs wrap its remainder.
    if let Some((_, placement)) = float_plan {
        para_layout.items.push(placement.item);
    }

    // The paragraph's content top in page coordinates (where the float image's
    // own top sits), captured before placement may advance/split the cursor.
    let para_top = state.cursor_y;
    let page_before = state.page_number;

    place_paragraph_layout(state, &resolved, para_layout, block_index);

    // Maintain the cross-paragraph float band.
    if state.page_number != page_before {
        // The paragraph crossed a page; wrap does not span pages.
        state.active_float = None;
    } else if let Some((inset, height, shift_text)) = own_float {
        // A float taller than its anchoring paragraph keeps wrapping below.
        let bottom_y = para_top + height;
        state.active_float =
            (bottom_y > state.cursor_y + 0.5).then_some(super::float_impl::ActiveFloat {
                bottom_y,
                inset,
                shift_text,
            });
    } else if let Some(af) = &state.active_float {
        // Inherited float: drop it once this paragraph reaches its bottom.
        if state.cursor_y >= af.bottom_y - 0.5 {
            state.active_float = None;
        }
    }

    if resolved.page_break_after && state.mode.is_paginated() {
        finish_page(state);
    }
}
