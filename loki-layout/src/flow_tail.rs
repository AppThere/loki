// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Section-finalization helpers for the flow engine (split from `flow.rs` for
//! the 300-line ceiling): the horizontal-rule block renderer, end-of-section
//! footnote rendering, the paragraph synthesizers used by the block loop for
//! bare/heading content, and the `get_items_max_x` content-width measurement
//! used by the table layout. The FlowState-touching entry points are
//! re-exported from `flow.rs`.

use loki_doc_model::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;

use super::{FlowState, editing, flow_block, flow_paragraph};
use crate::color::LayoutColor;
use crate::geometry::LayoutRect;
use crate::items::{PositionedItem, PositionedRect};
use crate::resolve::CollectedNote;

// ── Miscellaneous block renderers ─────────────────────────────────────────────

pub(super) fn flow_hrule(state: &mut FlowState) {
    const RULE_HEIGHT: f32 = 1.0;
    const RULE_SPACING: f32 = 6.0;
    state
        .current_items
        .push(PositionedItem::HorizontalRule(PositionedRect {
            rect: LayoutRect::new(0.0, state.cursor_y, state.content_width, RULE_HEIGHT),
            color: LayoutColor::BLACK,
        }));
    state.cursor_y += RULE_HEIGHT + RULE_SPACING;
}

// ── Footnote rendering ────────────────────────────────────────────────────────

/// Separator geometry above a page's first footnote (0.5 pt rule, 4 pt of gap
/// above and below), reserved once per page.
const SEP_HEIGHT: f32 = 0.5;
const SEP_GAP: f32 = 4.0;
const SEP_BAND: f32 = SEP_GAP + SEP_HEIGHT + SEP_GAP;

/// Measure the foot-of-page space `notes` need on the current page — the
/// separator band (only on the page's first reservation) plus each note's
/// laid-out height. **Pure** (no mutation), so the caller applies it only once
/// the reference paragraph is placed on this page. `0.0` when there is nothing
/// to reserve or the flow is multi-column (footnotes are single-column only).
pub(super) fn footnote_reservation(state: &mut FlowState, notes: &[CollectedNote]) -> f32 {
    if notes.is_empty() || state.columns != 1 {
        return 0.0;
    }
    let sep = if state.footnote_reserved == 0.0 {
        SEP_BAND
    } else {
        0.0
    };
    sep + notes
        .iter()
        .map(|n| measure_note_height(state, n))
        .sum::<f32>()
}

/// Height one footnote will occupy, laid out (with its reference mark) exactly
/// as [`render_footnote_bodies`] will render it, so the reserved band matches.
fn measure_note_height(state: &mut FlowState, note: &CollectedNote) -> f32 {
    let mark = format!("{} ", footnote_mark(note.number));
    let mut h = 0.0;
    let mut first = true;
    for block in &note.blocks {
        if let Block::StyledPara(p) = block {
            let mut p = p.clone();
            if first {
                p.inlines.insert(0, Inline::Str(mark.clone()));
            }
            first = false;
            let resolved = crate::resolve::resolve_para_props(&p, state.catalog);
            let mut counter = state.note_counter;
            let (text, spans, _images, _notes) = crate::resolve::flatten_paragraph_with_base(
                &p,
                state.catalog,
                &mut counter,
                None,
                state.options.revision_display,
            );
            let layout = crate::para::layout_paragraph_spelled(
                state.resources,
                &text,
                &spans,
                &resolved,
                state.content_width,
                state.display_scale,
                false,
                None,
            );
            h += resolved.space_before + layout.height + resolved.space_after;
        }
    }
    h
}

/// Lay out the current page's footnotes at the foot of the page. Called by
/// `finish_page` before the page is finalized (so each footnote sits on the page
/// carrying its reference, matching Word). Single-column body flow only.
///
/// The band is bottom-aligned at `page_content_height − total`, but never above
/// where content stopped (`cursor_y`). Body content already stops above it via
/// the per-reference reservation ([`footnote_reservation`] +
/// [`FlowState::content_bottom`]); this bottom-aligns the actual render.
/// Pagination is disabled during rendering (the band is self-contained) and a
/// re-entrancy guard blocks recursion.
pub(super) fn flow_page_footnotes(state: &mut FlowState) {
    if state.pending_footnotes.is_empty() || state.rendering_footnotes || state.columns != 1 {
        return;
    }
    let notes = std::mem::take(&mut state.pending_footnotes);
    let total: f32 = SEP_BAND
        + notes
            .iter()
            .map(|n| measure_note_height(state, n))
            .sum::<f32>();
    let band_top = (state.page_content_height - total).max(state.cursor_y);
    state.cursor_y = band_top;
    state.rendering_footnotes = true;
    // Disable pagination for the self-contained band (avoids a spurious break /
    // finish_page recursion if the measured height rounds under the rendered).
    let saved = state.page_content_height;
    state.page_content_height = f32::MAX;
    render_footnote_bodies(state, notes);
    state.page_content_height = saved;
    state.rendering_footnotes = false;
}

/// Render any remaining footnotes at the current position — the non-paginated
/// (canvas / reflow) tail, which has no per-page bands to place them in.
pub(super) fn flow_footnotes(state: &mut FlowState) {
    if state.pending_footnotes.is_empty() {
        return;
    }
    let notes = std::mem::take(&mut state.pending_footnotes);
    render_footnote_bodies(state, notes);
}

/// Emit the separator rule and each note body from `state.cursor_y` downward.
fn render_footnote_bodies(state: &mut FlowState, notes: Vec<CollectedNote>) {
    let sep_w = state.content_width / 3.0;
    state.cursor_y += SEP_GAP;
    state
        .current_items
        .push(PositionedItem::HorizontalRule(PositionedRect {
            rect: LayoutRect::new(0.0, state.cursor_y, sep_w, SEP_HEIGHT),
            color: LayoutColor::BLACK,
        }));
    state.cursor_y += SEP_HEIGHT + SEP_GAP;

    for note in notes {
        let mark = format!("{} ", footnote_mark(note.number));
        let mut first = true;
        for (body_block, block) in note.blocks.iter().enumerate() {
            // Tag body paragraph(s) so a click into the footnote resolves to the
            // live note-body container.
            state.nested_editing = Some(editing::NestedEditing::note(
                note.owner_block_index,
                note.note_in_block,
                body_block,
            ));
            if first {
                first = false;
                if let Block::StyledPara(p) = block {
                    let mut p = p.clone();
                    p.inlines.insert(0, Inline::Str(mark.clone()));
                    flow_paragraph(state, &p, 0);
                    continue;
                }
            }
            flow_block(state, block, 0);
        }
    }
    state.nested_editing = None;
}

/// Return the Unicode superscript mark for note number `n`.
fn footnote_mark(n: u32) -> String {
    match n {
        1 => "\u{00B9}".to_string(),
        2 => "\u{00B2}".to_string(),
        3 => "\u{00B3}".to_string(),
        4 => "\u{2074}".to_string(),
        5 => "\u{2075}".to_string(),
        6 => "\u{2076}".to_string(),
        7 => "\u{2077}".to_string(),
        8 => "\u{2078}".to_string(),
        9 => "\u{2079}".to_string(),
        _ => format!("[{n}]"),
    }
}

// ── Paragraph synthesisers ────────────────────────────────────────────────────

pub(super) fn synthesize_plain_para(inlines: &[Inline]) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

pub(super) fn synthesize_heading_para(
    level: u8,
    attr: &NodeAttr,
    inlines: &[Inline],
) -> StyledParagraph {
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
    // Prefer the style name carried in NodeAttr (set by the ODF mapper from
    // text:style-name so the catalog can resolve ODF heading properties like
    // font-size and bold). Fall back to the canonical OOXML/internal names.
    let style_id: StyleId = attr
        .kv
        .iter()
        .find(|(k, _)| k == "style")
        .map(|(_, v)| StyleId::new(v.as_str()))
        .unwrap_or_else(|| {
            let hardcoded = match level {
                1 => "Heading1",
                2 => "Heading2",
                3 => "Heading3",
                4 => "Heading4",
                5 => "Heading5",
                _ => "Heading6",
            };
            StyleId::new(hardcoded)
        });
    let direct_alignment =
        attr.kv
            .iter()
            .find(|(k, _)| k == "jc")
            .and_then(|(_, v)| match v.as_str() {
                "center" => Some(ParagraphAlignment::Center),
                "right" => Some(ParagraphAlignment::Right),
                "justify" => Some(ParagraphAlignment::Justify),
                _ => None,
            });
    let direct_para_props = direct_alignment.map(|align| {
        Box::new(ParaProps {
            alignment: Some(align),
            ..Default::default()
        })
    });
    StyledParagraph {
        style_id: Some(style_id),
        direct_para_props,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

// ── Table layout ─────────────────────────────────────────────────────────────

pub(super) fn get_items_max_x(items: &[PositionedItem]) -> f32 {
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
            PositionedItem::HatchRect(h) => h.rect.origin.x + h.rect.size.width,
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
