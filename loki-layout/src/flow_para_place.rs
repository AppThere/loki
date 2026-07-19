// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph placement into the flow, split out of `flow_para.rs` for the
//! 300-line ceiling: `place_paragraph_layout` handles continuous vs paginated
//! placement, keep-together, and line-boundary splitting. Re-exported from the
//! parent (`para_impl`) for `flow_paragraph` and the `chain` submodule.

use std::sync::Arc;

use super::{
    FlowState, LayoutWarning, ParagraphLayout, ResolvedParaProps, break_column, push_editing_para,
    split_and_place_loop,
};

/// Place a pre-computed paragraph layout, handling `keep_together` and splitting.
///
/// `space_before` must already be reflected in `state.cursor_y` by the caller.
///
/// # Errors
///
/// Non-fatal issues are pushed onto `state.warnings` rather than returned.
pub(super) fn place_paragraph_layout(
    state: &mut FlowState,
    resolved: &ResolvedParaProps,
    para_layout: ParagraphLayout,
    block_index: usize,
) {
    if !state.mode.is_paginated() {
        let dy = state.cursor_y;
        let dx = state.current_indent;
        if state.options.preserve_for_editing {
            // origin (dx, dy) matches the item translation below (lists indent dx).
            push_editing_para(state, block_index, Arc::new(para_layout.clone()), (dx, dy));
        }
        for mut item in para_layout.items {
            item.translate(dx, dy);
            state.current_items.push(item);
        }
        state.cursor_y += para_layout.height + resolved.space_after;
        return;
    }

    // keep-together: attempt to place all lines on one page (ADR 004 §4).
    if resolved.keep_together {
        let needed = para_layout.height + resolved.space_after;
        if needed > state.page_content_height {
            // Block exceeds full page height; cannot keep together anywhere.
            state.warnings.push(LayoutWarning::KeepTogetherOverride {
                block_index,
                block_height: para_layout.height,
            });
            tracing::warn!(
                block_index,
                block_height = para_layout.height,
                "keep-together paragraph exceeds page height; splitting"
            );
            // Fall through to normal splitting.
        } else {
            let available = state.page_content_height - state.cursor_y;
            if needed > available && state.cursor_y > 0.0 {
                break_column(state);
                state.cursor_y += resolved.space_before;
            }
            // Fits on current (or freshly flushed) column/page.
            let dy = state.cursor_y;
            let dx = state.current_indent;
            if state.options.preserve_for_editing {
                push_editing_para(state, block_index, Arc::new(para_layout.clone()), (0.0, dy));
            }
            super::super::line_numbers::emit(state, &para_layout, dy, 0.0, para_layout.height);
            for mut item in para_layout.items {
                item.translate(dx, dy);
                state.current_items.push(item);
            }
            state.cursor_y += para_layout.height + resolved.space_after;
            return;
        }
    }

    // Normal paginated placement with line-boundary splitting.
    if para_layout.height > state.page_content_height {
        state.warnings.push(LayoutWarning::BlockExceedsPageHeight {
            block_index,
            block_height: para_layout.height,
        });
    }

    let dx = state.current_indent;
    let arc_layout = if state.options.preserve_for_editing {
        Some(Arc::new(para_layout.clone()))
    } else {
        None
    };
    split_and_place_loop(state, resolved, &para_layout, arc_layout, block_index, dx);
    state.cursor_y += resolved.space_after;
}
