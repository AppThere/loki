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

use crate::para::{ParagraphLayout, ResolvedParaProps, layout_paragraph_spelled};
use crate::resolve::resolve_para_props;

use super::columns_impl::break_column;
use super::editing::push_editing_para;
use super::{FlowState, LayoutWarning, finish_page};

#[path = "flow_para_chain.rs"]
mod chain;
#[path = "flow_para_images.rs"]
mod images;
#[path = "flow_para_place.rs"]
mod place;
#[path = "flow_split.rs"]
mod split;
#[path = "flow_widow_orphan.rs"]
mod widow_orphan;

pub(super) use chain::flow_keep_with_next_chain;
pub(super) use images::{apply_overlay_images, stack_block_images};
use place::{place_paragraph_layout, place_with_footnote_band};
use split::split_and_place_loop;

// ── Public(super) API ─────────────────────────────────────────────────────────

/// Resolve, lay out, and place a single paragraph block.
pub(super) fn flow_paragraph(state: &mut FlowState, para: &StyledParagraph, block_index: usize) {
    let mut resolved = resolve_para_props(para, state.catalog);
    // A tracked ¶-mark deletion paints a struck end-of-paragraph marker only in
    // the All-Markup view; Final/Original render the accepted/rejected document,
    // where no revision decoration is shown. (Full paragraph *merge* in the
    // Final view is not modelled — the ¶ is simply not marked; see the
    // `revision_filter` module docs.)
    if state.options.revision_display != crate::options::RevisionDisplay::AllMarkup {
        resolved.para_mark_deleted_color = None;
    }
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

    // List level indentation fallback (numbering `pPr` indent when the paragraph
    // carries none) — extracted to `flow_list_marker` for the 300-line ceiling.
    super::flow_list_marker::apply_level_indent_fallback(state, &mut resolved);

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
        state.options.revision_display,
    );
    // Tag each note with its owning block + per-block order. The notes render at
    // the foot of the page carrying their reference — see `flow_tail`.
    for (i, note) in notes.iter_mut().enumerate() {
        note.owner_block_index = block_index;
        note.note_in_block = i;
    }
    // Measure this paragraph's footnote band now; `place_with_footnote_band`
    // applies it after placement (shrinking `content_bottom()` for following
    // content) iff the paragraph stays on `page_before_para`.
    let footnote_reserve = super::tail::footnote_reservation(state, &notes);
    let page_before_para = state.page_number;
    state.pending_footnotes.extend(notes);

    // Floating image/text-box wrap (gap #12): plan the paragraph's own float,
    // set its wrap band on `resolved`, and drop the floated image from the
    // block-stacked set (see `flow_float::plan_paragraph_float`).
    let (float_plan, own_float) =
        super::float_impl::plan_paragraph_float(state, &mut images, &mut resolved);

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

    // ── Flow-level item injection ────────────────────────────────────────────
    // Picture bullets, inline images and floats are pushed into the paragraph's
    // items *after* shaping, so they cannot live in the shared cache entry.
    //
    // Each is conditional and the overwhelming majority of paragraphs need none
    // of them, so the copy is taken only when there is something to inject
    // (S9-1): `Arc::make_mut` clones here, since the cache always holds a
    // second reference, and the resulting private copy is what reaches both the
    // page items and the editing index — exactly the layout that was placed.
    // Paragraphs that skip this block keep the single shared allocation.
    //
    // Picture bullet (feature 5.4): place the label image in the hanging label
    // box on line 0. Injected into the paragraph's items so it translates with
    // the paragraph on placement.
    let bullet_item = marker
        .bullet_src
        .as_ref()
        .and_then(|src| super::flow_list_marker::picture_bullet_item(src, &resolved, &para_layout));
    if bullet_item.is_some() || !images.is_empty() || float_plan.is_some() {
        let layout = std::sync::Arc::make_mut(&mut para_layout);
        if let Some(item) = bullet_item {
            layout.items.push(item);
        }

        // ── Inline image placement (gap #9) ──────────────────────────────────
        // Block-stack the non-floating images and collect any `wrapNone` overlays.
        let overlay_items = stack_block_images(
            layout,
            &images,
            state.content_width,
            state.mode.fits_oversized_to_column(),
        );

        // Emit the float beside the wrapped text; a float taller than its text
        // becomes an `ActiveFloat` so *following* paragraphs wrap its remainder.
        if let Some((_, placement)) = float_plan {
            layout.items.push(placement.item);
        }

        // Emit overlay (`wrapNone`) floats last: behind-text ones go under the
        // whole paragraph (drawn first), in-front ones over the text (drawn last).
        // Neither reserves vertical space nor shifts the text.
        apply_overlay_images(layout, overlay_items);
    }

    // The paragraph's content top in page coordinates (where the float image's
    // own top sits), captured before placement may advance/split the cursor.
    let para_top = state.cursor_y;
    let page_before = state.page_number;

    // Place the paragraph, exempting an empty one from — and otherwise applying —
    // this page's footnote-band reservation (see `place_with_footnote_band`).
    place_with_footnote_band(
        state,
        &resolved,
        para_layout,
        block_index,
        text.trim().is_empty(),
        footnote_reserve,
        page_before_para,
    );

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
