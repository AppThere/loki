// SPDX-License-Identifier: Apache-2.0

//! The Insert tab's **Breaks** group: page break, paragraph break, line
//! break, non-breaking space, and horizontal rule.
//!
//! Each control reuses the machinery its keyboard twin already exercises,
//! so the button and the key cannot drift:
//! - **Paragraph break** *is* the Enter path (`handle_enter_key` — including
//!   next-style and list-exit semantics).
//! - **Line break** and **non-breaking space** go through the character
//!   insertion path (`handle_character_key`) with `"\n"` / U+00A0 — a line
//!   break is stored as a newline in the block text, which the bridge reads
//!   back as `Inline::LineBreak`.
//! - **Page break** splits at the caret like Enter, then flags the following
//!   block `page_break_before` (the model's hard-break expression). Top-level
//!   carets only: the layout starts pages at top-level blocks, so a break
//!   inside a table cell or note body would be a silent no-op — declined with
//!   a status message instead.
//! - **Horizontal rule** inserts `Block::HorizontalRule` after the caret's
//!   top-level block (after the enclosing table/frame when the caret is
//!   nested — the rule is a block, so between blocks is where it can go).

use appthere_ui::{
    AT_NBSP, AT_PAGE_BREAK, AtIcon, AtRibbonIconButton, LUCIDE_CORNER_DOWN_LEFT, LUCIDE_MINUS,
    LUCIDE_PILCROW, RibbonGroupSpec, estimate_group_metrics,
};
use dioxus::prelude::*;
use loki_doc_model::content::block::Block;
use loki_doc_model::loro_mutation::{insert_block_after, set_block_page_break_before, split_block};
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_enter::handle_enter_key;
use super::editor_keydown_text::{handle_character_key, set_collapsed_cursor};
use super::editor_ribbon_insert::InsertCtx;
use super::editor_state::SaveStatus;
use crate::editing::cursor::DocumentPosition;
use crate::editing::state::apply_mutation_and_relayout;

/// The caret focus, or a status message when there is none.
fn focus_or_report(ctx: &InsertCtx) -> Option<DocumentPosition> {
    let focus = ctx.cursor_state.read().focus.clone();
    if focus.is_none() {
        let mut save_message = ctx.save_message;
        save_message.set(Some(SaveStatus::error(fl!("editor-insert-no-cursor"))));
    }
    focus
}

/// Paragraph break — exactly the Enter key.
pub(super) fn insert_paragraph_break(ctx: &InsertCtx) {
    let Some(focus) = focus_or_report(ctx) else {
        return;
    };
    handle_enter_key(
        focus,
        ctx.loro_doc,
        &ctx.doc_state,
        ctx.cursor_state,
        ctx.undo_manager,
        ctx.can_undo,
        ctx.can_redo,
    );
}

/// Line break / non-breaking space — the character path with a fixed string.
pub(super) fn insert_char(ctx: &InsertCtx, ch: &str) {
    let Some(focus) = focus_or_report(ctx) else {
        return;
    };
    handle_character_key(
        ch.to_string(),
        focus,
        ctx.loro_doc,
        &ctx.doc_state,
        ctx.cursor_state,
        ctx.undo_manager,
        ctx.can_undo,
        ctx.can_redo,
    );
}

/// Page break: split at the caret, flag the following block, land the caret
/// at its start. Declines (with the reason) for nested carets — see the
/// module docs.
pub(super) fn insert_page_break(ctx: &InsertCtx) {
    let mut save_message = ctx.save_message;
    let Some(focus) = focus_or_report(ctx) else {
        return;
    };
    if !focus.path.is_empty() {
        save_message.set(Some(SaveStatus::error(fl!("editor-page-break-nested"))));
        return;
    }
    let ok = {
        let guard = ctx.loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        let split = split_block(ldoc, focus.paragraph_index, focus.byte_offset).is_ok()
            && set_block_page_break_before(ldoc, focus.paragraph_index + 1, true).is_ok();
        if split {
            apply_mutation_and_relayout(&ctx.doc_state, ldoc);
        }
        split
    };
    if ok {
        post_mutation_sync(
            &ctx.doc_state,
            ctx.loro_doc,
            ctx.cursor_state,
            ctx.undo_manager,
            ctx.can_undo,
            ctx.can_redo,
        );
        // The caret lands at the start of the page-broken block; the page
        // index is re-derived from the fresh layout.
        set_collapsed_cursor(
            &ctx.doc_state,
            ctx.cursor_state,
            DocumentPosition::top_level(focus.page_index, focus.paragraph_index + 1, 0),
        );
    } else {
        save_message.set(Some(SaveStatus::error(fl!("editor-insert-failed"))));
    }
}

/// Horizontal rule after the caret's top-level block.
pub(super) fn insert_horizontal_rule(ctx: &InsertCtx) {
    let mut save_message = ctx.save_message;
    let Some(focus) = focus_or_report(ctx) else {
        return;
    };
    let ok = {
        let guard = ctx.loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        let inserted =
            insert_block_after(ldoc, focus.paragraph_index, &Block::HorizontalRule).is_ok();
        if inserted {
            apply_mutation_and_relayout(&ctx.doc_state, ldoc);
        }
        inserted
    };
    if ok {
        post_mutation_sync(
            &ctx.doc_state,
            ctx.loro_doc,
            ctx.cursor_state,
            ctx.undo_manager,
            ctx.can_undo,
            ctx.can_redo,
        );
    } else {
        save_message.set(Some(SaveStatus::error(fl!("editor-insert-failed"))));
    }
}

/// One Breaks button: icon path, accessible label, action.
type BreakButton = (&'static str, String, fn(&InsertCtx));

/// Builds the Breaks group (see the module docs). Priority 4 in the Insert
/// tab's collision-free ladder (media 5 > breaks 4 > tables 3 > page 2 >
/// references 1 > links 0).
pub(super) fn breaks_group(ctx: InsertCtx) -> RibbonGroupSpec {
    let buttons: [BreakButton; 5] = [
        (AT_PAGE_BREAK, fl!("ribbon-insert-page-break-aria"), |c| {
            insert_page_break(c)
        }),
        (LUCIDE_PILCROW, fl!("ribbon-insert-para-break-aria"), |c| {
            insert_paragraph_break(c)
        }),
        (
            LUCIDE_CORNER_DOWN_LEFT,
            fl!("ribbon-insert-line-break-aria"),
            |c| insert_char(c, "\n"),
        ),
        (AT_NBSP, fl!("ribbon-insert-nbsp-aria"), |c| {
            insert_char(c, "\u{a0}")
        }),
        (LUCIDE_MINUS, fl!("ribbon-insert-hrule-aria"), |c| {
            insert_horizontal_rule(c)
        }),
    ];
    RibbonGroupSpec {
        metrics: estimate_group_metrics(4, 5, true),
        partial: None,
        label: Some(fl!("ribbon-group-breaks")),
        aria_label: fl!("ribbon-group-breaks"),
        content: rsx! {
            for (icon, aria, action) in buttons.into_iter() {
                AtRibbonIconButton {
                    key: "{aria}",
                    aria_label: aria.clone(),
                    is_active: false,
                    is_disabled: false,
                    on_click: {
                        let ctx = ctx.clone();
                        move |_| action(&ctx)
                    },
                    AtIcon { path_d: icon.to_string() }
                }
            }
        },
    }
}
