// SPDX-License-Identifier: Apache-2.0

//! The Insert tab's section-break action (§3d UI hookup — split from
//! `editor_ribbon_insert.rs` for the 300-line ceiling).

use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::super::editor_state::SaveStatus;
use super::InsertCtx;
use crate::editing::state::apply_mutation_and_relayout;

/// The section-break click: resolve the caret's section, insert a new one
/// after it, relayout, and report through the status banner — the same
/// outcome shape as the other Insert actions.
pub(super) fn insert_section_break(ctx: &InsertCtx) {
    let mut save_message = ctx.save_message;
    let outcome = {
        let guard = ctx.loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        let caret = ctx
            .cursor_state
            .read()
            .focus
            .as_ref()
            .map(|f| f.paragraph_index);
        let Some(block_index) = caret else {
            save_message.set(Some(SaveStatus::error(fl!("editor-insert-no-cursor"))));
            return;
        };
        let Some(section) = loki_doc_model::loro_mutation::section_of_block(ldoc, block_index)
        else {
            save_message.set(Some(SaveStatus::error(fl!("editor-insert-failed"))));
            return;
        };
        let ok = loki_doc_model::loro_mutation::insert_section_after(ldoc, section).is_ok();
        if ok {
            apply_mutation_and_relayout(&ctx.doc_state, ldoc);
        }
        ok
    };
    if outcome {
        post_mutation_sync(
            &ctx.doc_state,
            ctx.loro_doc,
            ctx.cursor_state,
            ctx.undo_manager,
            ctx.can_undo,
            ctx.can_redo,
        );
        save_message.set(Some(SaveStatus::ok(fl!("editor-insert-section-ok"))));
    } else {
        save_message.set(Some(SaveStatus::error(fl!("editor-insert-failed"))));
    }
}
