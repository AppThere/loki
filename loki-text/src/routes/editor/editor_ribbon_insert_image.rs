// SPDX-License-Identifier: Apache-2.0

//! Insert → Image: the async pick → read → embed → insert flow.
//!
//! Split out of [`super::editor_ribbon_insert`] to keep that module under the
//! 300-line ceiling. Opens the platform file picker, reads the chosen file,
//! builds a data-URI `Inline::Image`, inserts it at the cursor, and reports the
//! outcome through the status banner.

use std::io::Read;

use super::editor_state::SaveStatus;
use dioxus::prelude::*;
use loki_file_access::{FileAccessToken, FilePicker, PickOptions};
use loki_i18n::fl;

use super::editor_insert::{image_inline_from_bytes, insert_image_at_cursor};
use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_ribbon_insert::InsertCtx;
use crate::editing::state::apply_mutation_and_relayout;

/// Raster image MIME types offered in the Insert → Image picker.
const IMAGE_MIME_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/bmp",
];

/// Reads all bytes from a picked file token.
fn read_token_bytes(token: &FileAccessToken) -> std::io::Result<Vec<u8>> {
    let mut reader = token
        .open_read()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Spawns the async pick → embed → insert flow for an image at the cursor.
///
/// Opens the platform file picker, reads the chosen file, builds a data-URI
/// `Inline::Image`, inserts it at the cursor, and reports the outcome through
/// the status banner. A cancelled picker is a silent no-op.
pub(super) fn spawn_pick_and_insert_image(ctx: InsertCtx) {
    let mut save_message = ctx.save_message;
    spawn(async move {
        let picker = FilePicker::new();
        let opts = PickOptions {
            mime_types: IMAGE_MIME_TYPES.iter().map(|s| (*s).to_string()).collect(),
            filter_label: Some(fl!("ribbon-insert-image-filter")),
            multi: false,
        };
        match picker.pick_file_to_open(opts).await {
            Ok(Some(token)) => {
                let bytes = match read_token_bytes(&token) {
                    Ok(b) => b,
                    Err(e) => {
                        save_message.set(Some(SaveStatus::error(fl!(
                            "editor-insert-image-error",
                            reason = e.to_string()
                        ))));
                        return;
                    }
                };
                let Some(image) = image_inline_from_bytes(&bytes) else {
                    save_message.set(Some(SaveStatus::error(fl!(
                        "editor-insert-image-unsupported"
                    ))));
                    return;
                };
                let outcome = {
                    let guard = ctx.loro_doc.read();
                    match guard.as_ref() {
                        Some(ldoc) => {
                            match insert_image_at_cursor(ldoc, &ctx.cursor_state.read(), &image) {
                                Ok(true) => {
                                    apply_mutation_and_relayout(&ctx.doc_state, ldoc);
                                    Some(true)
                                }
                                Ok(false) => Some(false),
                                Err(_) => None,
                            }
                        }
                        None => None,
                    }
                };
                match outcome {
                    Some(true) => {
                        post_mutation_sync(
                            &ctx.doc_state,
                            ctx.loro_doc,
                            ctx.cursor_state,
                            ctx.undo_manager,
                            ctx.can_undo,
                            ctx.can_redo,
                        );
                        save_message.set(Some(SaveStatus::ok(fl!("editor-insert-image-success"))));
                    }
                    Some(false) => save_message.set(Some(SaveStatus::error(fl!(
                        "editor-insert-image-no-cursor"
                    )))),
                    None => save_message.set(Some(SaveStatus::error(fl!(
                        "editor-insert-image-error",
                        reason = "—"
                    )))),
                }
            }
            Ok(None) => { /* user cancelled — no-op */ }
            Err(e) => save_message.set(Some(SaveStatus::error(fl!(
                "editor-insert-image-error",
                reason = e.to_string()
            )))),
        }
    });
}
