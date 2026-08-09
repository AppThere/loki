// SPDX-License-Identifier: Apache-2.0

//! Ordered and bullet lists for the DOM reflow view.
//!
//! # Not `<ul>` / `<ol>`
//!
//! A browser list produces its own markers at its own indent, and the canvas
//! path produces different ones: `loki_layout` prefixes the marker **into the
//! item's first paragraph** and gives that paragraph a hanging indent. Letting
//! CSS do it would be a second answer to "which marker, at what indent", and the
//! two views would disagree about a list for a reason that is not rendering.
//!
//! So this walks the same structure with the same three rules, taken from
//! `loki_layout` rather than restated: [`loki_layout::flow::list_marker`] for the
//! marker, [`loki_layout::flow::NESTED_INDENT_PT`] for the step, and
//! [`loki_layout::flow::synthesize_list_item_para`] for the item's first
//! paragraph. The result goes through the same paragraph renderer as body text.
//!
//! # Where the two still disagree, and why it is not this module
//!
//! A list item's **non-first** blocks are laid out by the canvas path at the
//! *full* column width and merely translated right by the item's indent
//! (`FlowState::current_indent` is a placement offset, not a measure), so its
//! text overruns the column by one step. This module gives them the indent as
//! padding, which narrows them — so the second paragraph of an item wraps later
//! here, correctly. `TODO(list-indent-measure)` on `loki_layout`'s
//! `flow_list_item` carries the canvas-side fix; see ADR-0017 §5.9.
//!
//! # The hanging indent is a box, because it cannot be a property
//!
//! On the canvas path the marker ends in a tab and the paragraph hangs by one
//! step, so the tab carries the text from the marker to the item's own indent.
//! Neither half of that survives here — this stack implements neither
//! `text-indent` nor a tab stop (measured; see
//! [`super::style::hanging_row_css`], which is what replaces them). The first
//! version of this module set `tab-size` in points and trusted the hanging
//! indent, and the swept comparison disagreed with the canvas path at 24 of 36
//! widths.
//!
//! # Not covered
//!
//! `ListAttributes::style` and its delimiter: every ordered list numbers `1.`,
//! `2.`, … whatever the document says. That is the canvas path's behaviour and
//! `TODO(list-number-style)` on `list_marker` carries it — changing it here
//! alone would make the two views disagree, which is the thing this module is
//! arranged to prevent.

use dioxus::prelude::*;
use loki_doc_model::content::block::{Block, ListAttributes};
use loki_doc_model::style::catalog::StyleCatalog;

use super::content::{FamilyMap, block_el};
use super::content_para::hanging_marker_para_el;

/// One list, ordered when `attrs` is `Some`.
///
/// `indent` is the enclosing indent in points — a nested list is rendered by the
/// recursion below with its parent's already added, the same accumulation
/// `FlowState::current_indent` performs.
pub(super) fn list_el(
    attrs: Option<&ListAttributes>,
    items: &[Vec<Block>],
    indent: f32,
    catalog: &StyleCatalog,
    families: &FamilyMap,
) -> Element {
    let list_indent = indent + loki_layout::flow::NESTED_INDENT_PT;
    rsx! {
        div {
            for (i, item) in items.iter().enumerate() {
                {
                    let marker = loki_layout::flow::list_marker(attrs, i);
                    rsx! {
                        div {
                            key: "{i}",
                            { item_el(item, &marker, list_indent, catalog, families) }
                        }
                    }
                }
            }
        }
    }
}

/// One item: the marker rides on the first paragraph, the rest of the blocks sit
/// at the item's indent.
fn item_el(
    item: &[Block],
    marker: &str,
    list_indent: f32,
    catalog: &StyleCatalog,
    families: &FamilyMap,
) -> Element {
    rsx! {
        for (i, block) in item.iter().enumerate() {
            {
                match (i, block) {
                    // Only a `StyledPara` can carry the marker, which is the
                    // canvas path's rule too: an item that opens with a table or
                    // a nested list gets no marker there either, and matching
                    // that matters more than a marker would.
                    (0, Block::StyledPara(p)) => {
                        let p = loki_layout::flow::synthesize_list_item_para(
                            p, marker, list_indent,
                        );
                        rsx! {
                            div {
                                key: "{i}",
                                { hanging_marker_para_el(&p, catalog, families, marker.len()) }
                            }
                        }
                    }
                    // A nested list is recursed into with **this item's** indent
                    // and no wrapper: its own `list_el` adds the step, so an
                    // indent here would be the step twice. The canvas path does
                    // the same — `flow_list_item` sets `current_indent` for the
                    // item's other blocks, and the nested list's synthesised
                    // paragraphs then carry the total themselves. Measured
                    // before the fix: a nested item's text sat 24 pt right of
                    // where the canvas put it.
                    (_, Block::OrderedList(attrs, items)) => rsx! {
                        div {
                            key: "{i}",
                            { list_el(Some(attrs), items, list_indent, catalog, families) }
                        }
                    },
                    (_, Block::BulletList(items)) => rsx! {
                        div {
                            key: "{i}",
                            { list_el(None, items, list_indent, catalog, families) }
                        }
                    },
                    // Everything else sits at the item's indent, which is the
                    // ambient one the canvas path flows it under.
                    _ => rsx! {
                        div {
                            key: "{i}",
                            style: format!("padding-inline-start: {list_indent}pt;"),
                            { block_el(block, catalog, families) }
                        }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
