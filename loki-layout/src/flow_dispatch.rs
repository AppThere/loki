// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Block dispatch and page finalization, split out of `flow.rs` for the
//! 300-line ceiling: `flow_block` routes each block variant to its handler
//! (lists synthesize marker paragraphs inline), `flow_blocks` recurses child
//! bodies, and `finish_page` positions columns and pushes a `LayoutPage`.
//! `flow_block` / `finish_page` are re-exported from `flow.rs`.

use loki_doc_model::content::block::{Block, ListAttributes, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_primitives::units::Points;

use crate::result::{LayoutPage, PageEditingData};

use super::{
    BreakCause, FlowState, columns_impl, comments_impl, float_impl, flow_hrule, flow_paragraph,
    para_between, synthesize_heading_para, synthesize_plain_para, table_main,
};

// ── Block dispatch ────────────────────────────────────────────────────────────

pub(crate) fn flow_block(state: &mut FlowState, block: &Block, idx: usize) {
    // Only consecutive plain paragraphs continue a cross-paragraph float wrap;
    // any other block clears the float (reserving its remaining height) so it
    // does not overlap the image.
    if !matches!(
        block,
        Block::StyledPara(_) | Block::Para(_) | Block::Plain(_) | Block::Heading(..)
    ) {
        float_impl::reserve_active_float(state);
    }
    match block {
        Block::StyledPara(p) => flow_paragraph(state, p, idx),
        Block::Para(i) | Block::Plain(i) => {
            flow_paragraph(state, &synthesize_plain_para(i), idx);
        }
        Block::Heading(lvl, attr, i) => {
            flow_paragraph(state, &synthesize_heading_para(*lvl, attr, i), idx);
        }
        Block::BulletList(items) => {
            let old_indent = state.current_indent;
            let list_indent = old_indent + NESTED_INDENT_PT;
            for (i, item) in items.iter().enumerate() {
                flow_list_item(state, item, &list_marker(None, i), list_indent, idx);
            }
            state.current_indent = old_indent;
        }
        Block::OrderedList(attrs, items) => {
            let old_indent = state.current_indent;
            let list_indent = old_indent + NESTED_INDENT_PT;
            for (i, item) in items.iter().enumerate() {
                flow_list_item(state, item, &list_marker(Some(attrs), i), list_indent, idx);
            }
            state.current_indent = old_indent;
        }
        Block::BlockQuote(blocks) => {
            let old_indent = state.current_indent;
            state.current_indent += NESTED_INDENT_PT;
            for b in blocks {
                flow_block(state, b, idx);
            }
            state.current_indent = old_indent;
        }
        Block::Div(_, blocks) | Block::Figure(_, _, blocks) => flow_blocks(state, blocks, idx),
        Block::Table(tbl) => table_main::flow_table(state, tbl, idx),
        Block::HorizontalRule => flow_hrule(state),
        Block::TableOfContents(toc) => flow_blocks(state, &toc.body, idx),
        Block::Index(index) => flow_blocks(state, &index.body, idx),
        _ => {}
    }
}

// ── List markers and indents ──────────────────────────────────────────────────
//
// `pub`, not private: ADR-0017's DOM reflow view renders the same lists, and a
// second answer to "which marker, at what indent" would make the two views
// disagree about a list for a reason that is not rendering. The synthesis is
// shared for the same reason `synthesize_heading_para` is.

/// The indent one list or block-quote level adds, in points.
pub const NESTED_INDENT_PT: f32 = 18.0;

/// The marker for item `index`, ordered or bulleted.
///
/// The trailing tab is what the hanging indent is measured against: the marker
/// sits in the hanging space and the tab carries the text to the item's own
/// indent.
///
/// `TODO(list-number-style)`: `ListAttributes::style` and its delimiter are not
/// read — every ordered list numbers `1.`, `2.`, … whatever the document says.
/// That is the canvas path's behaviour and this preserves it rather than
/// changing two views at once.
#[must_use]
pub fn list_marker(ordered: Option<&ListAttributes>, index: usize) -> String {
    match ordered {
        Some(attrs) => format!("{}.\t", attrs.start_number + index as i32),
        None => "\u{2022}\t".to_string(),
    }
}

/// A list item's first paragraph: the marker prefixed, hanging by the marker's
/// width, and the whole item indented.
#[must_use]
pub fn synthesize_list_item_para(
    para: &StyledParagraph,
    marker: &str,
    list_indent: f32,
) -> StyledParagraph {
    let mut p = para.clone();
    p.inlines.insert(0, Inline::Str(marker.to_string()));
    let mut direct = p.direct_para_props.take().unwrap_or_default();
    direct.indent_hanging = Some(Points::new(f64::from(NESTED_INDENT_PT)));
    direct.indent_start = Some(Points::new(f64::from(list_indent)));
    p.direct_para_props = Some(direct);
    p
}

/// Flows one list item: its first paragraph carries the marker, the rest of its
/// blocks sit at the item's indent.
///
/// # `TODO(list-indent-measure)`: the indent moves the block without narrowing it
///
/// `current_indent` is consumed by `flow_para_place` as a *translation* — the
/// paragraph is laid out against the full `state.content_width` and then shifted
/// right — so an item's non-first blocks overrun the column by `list_indent`.
/// Measured against the DOM reflow view, which gives them the indent as padding:
/// on a 565 px column the second paragraph of an item took two lines here and
/// three there, its longest line 423.6 pt against a 405.75 pt measure
/// (ADR-0017 §5.9). `BlockQuote` above has the same shape.
///
/// The fix is not `content_width -= current_indent`: the field means two things.
/// For a list or a quote it is an indent inside the current measure; for a table
/// cell (`flow_table_cells`) it is an absolute page x, set alongside a
/// `content_width` that is already the cell's. Separating those two is the
/// change, and it moves line breaks in every document with a quote or a
/// multi-block item — so it wants its own sweep, not a rider on this one.
fn flow_list_item(
    state: &mut FlowState,
    item: &[Block],
    marker: &str,
    list_indent: f32,
    idx: usize,
) {
    for (b_idx, b) in item.iter().enumerate() {
        if b_idx == 0
            && let Block::StyledPara(p) = b
        {
            let p = synthesize_list_item_para(p, marker, list_indent);
            // The synthesised paragraph carries the indent itself, so the
            // ambient one must not be added on top of it.
            let prev_indent = state.current_indent;
            state.current_indent = 0.0;
            flow_paragraph(state, &p, idx);
            state.current_indent = prev_indent;
            continue;
        }
        let prev_indent = state.current_indent;
        state.current_indent = list_indent;
        flow_block(state, b, idx);
        state.current_indent = prev_indent;
    }
}

/// Flows child blocks at the parent's `idx` (Div/Figure bodies, TOC/index snapshots).
fn flow_blocks(state: &mut FlowState, blocks: &[Block], idx: usize) {
    for (i, b) in blocks.iter().enumerate() {
        state.staged_between = para_between::stage(blocks, i, state.catalog);
        flow_block(state, b, idx);
    }
}

// ── Page management ───────────────────────────────────────────────────────────

/// Closes the current page. `cause` decides how paragraph spacing carries into
/// the next one — see [`BreakCause`].
pub(crate) fn finish_page(state: &mut FlowState, cause: BreakCause) {
    // Lay out this page's footnotes in the band reserved at their reference (per
    // the `pending_footnotes` doc). Runs before column positioning so the note
    // items are placed with the rest of the page's content.
    super::tail::flow_page_footnotes(state);

    // Position + separate the used columns, then reset for the next page.
    columns_impl::position_current_column(state);
    columns_impl::emit_column_separators(state);
    state.col_index = 0;
    state.column_top_y = 0.0;
    state.column_item_start = 0;
    state.column_para_start = 0;

    // Lay out the gutter comment panel for any comments anchored on this page.
    let comment_items = comments_impl::layout_comment_panel(state);

    let page = LayoutPage {
        page_number: state.page_number,
        page_size: state.page_size,
        margins: crate::paginate_blanks::mirrored_margins(
            state.margins,
            state.page_number,
            state.options.mirror_margins,
        ),
        content_items: std::mem::take(&mut state.current_items),
        header_items: vec![],
        footer_items: vec![],
        comment_items,
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: if state.options.preserve_for_editing {
            Some(PageEditingData {
                paragraphs: state.current_paragraphs.clone(),
            })
        } else {
            None
        },
    };
    state.pages.push(page);
    state.page_number += 1;
    state.current_paragraphs.clear();
    state.cursor_y = 0.0;
    // The footnote band is laid out; release its per-page reservation so the
    // fresh page starts with the full content height.
    state.footnote_reserved = 0.0;
    // Restart margin line numbering at the top of the new page (`newPage`).
    if let Some(ln) = &mut state.line_num {
        ln.restart_for_page();
    }
    // Cross-paragraph float wrap does not continue onto the next page.
    state.active_float = None;
    // Paragraph-spacing collapsing carries across only a *forced* break: after
    // a flow break the block whose `space_after` is pending is on the page just
    // closed, so collapsing against it would pull the next block's first line
    // up against the top margin.
    state.end_page_at(cause);
}
