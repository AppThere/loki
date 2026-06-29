// SPDX-License-Identifier: Apache-2.0

//! Insert tab ribbon content for the document editor (Spec 04 M4).
//!
//! [`insert_tab_content`] returns the `Element` passed to
//! [`AtRibbon::tab_content`] when the Insert tab is active. It hosts the create
//! controls for objects with a native Loro mapping that are useful without the
//! deferred cell/note interior-editing work:
//!
//! - **Image** — picks a file, embeds it as a `data:` URI, and inserts an
//!   `Inline::Image` at the cursor (see [`super::editor_insert`]).
//! - **Link** — opens the URL panel ([`super::editor_insert_panel`]).
//!
//! Table and footnote controls arrive once the mutation layer can address
//! content nested inside cells / note bodies — no control is shown before it
//! does something.

use std::io::Read;
use std::sync::{Arc, Mutex};

use appthere_ui::{AtIcon, AtRibbonGroup, AtRibbonIconButton, LUCIDE_IMAGE, LUCIDE_LINK};
use dioxus::prelude::*;
use loki_file_access::{FileAccessToken, FilePicker, PickOptions};
use loki_i18n::fl;

use super::editor_insert::{image_inline_from_bytes, insert_image_at_cursor};
use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Raster image MIME types offered in the Insert → Image picker.
const IMAGE_MIME_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/bmp",
];

/// Live-document handles the Insert tab needs to create objects at the cursor.
#[derive(Clone)]
pub(super) struct InsertCtx {
    /// Document state for relayout after a mutation.
    pub doc_state: Arc<Mutex<DocumentState>>,
    /// The document's Loro CRDT handle.
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    /// Cursor state — the insertion point and dirty-tracking generation.
    pub cursor_state: Signal<CursorState>,
    /// Undo manager, refreshed after the mutation.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether undo is available.
    pub can_undo: Signal<bool>,
    /// Whether redo is available.
    pub can_redo: Signal<bool>,
    /// Status-banner sink for success / error feedback.
    pub save_message: Signal<Option<String>>,
}

/// Builds the Insert tab ribbon content element.
///
/// `link_draft` is the shared hyperlink-panel signal (setting it to `Some("")`
/// opens the URL panel); `ctx` carries the handles the Image button needs to
/// pick a file and insert it at the cursor.
pub(super) fn insert_tab_content(
    mut link_draft: Signal<Option<String>>,
    ctx: InsertCtx,
) -> Element {
    rsx! {
        // ── Media group ───────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-media")),
            aria_label: fl!("ribbon-group-media"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-insert-image-aria"),
                is_active:   false,
                is_disabled: false,
                on_click: move |_| spawn_pick_and_insert_image(ctx.clone()),
                AtIcon { path_d: LUCIDE_IMAGE.to_string() }
            }
        }

        // ── Links group ───────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-links")),
            aria_label: fl!("ribbon-group-links"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-insert-link-aria"),
                is_active:   link_draft.read().is_some(),
                is_disabled: false,
                on_click: move |_| {
                    if link_draft.read().is_some() {
                        link_draft.set(None);
                    } else {
                        link_draft.set(Some(String::new()));
                    }
                },
                AtIcon { path_d: LUCIDE_LINK.to_string() }
            }
        }
    }
}

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
fn spawn_pick_and_insert_image(ctx: InsertCtx) {
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
                        save_message.set(Some(fl!(
                            "editor-insert-image-error",
                            reason = e.to_string()
                        )));
                        return;
                    }
                };
                let Some(image) = image_inline_from_bytes(&bytes) else {
                    save_message.set(Some(fl!("editor-insert-image-unsupported")));
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
                        save_message.set(Some(fl!("editor-insert-image-success")));
                    }
                    Some(false) => save_message.set(Some(fl!("editor-insert-image-no-cursor"))),
                    None => save_message.set(Some(fl!("editor-insert-image-error", reason = "—"))),
                }
            }
            Ok(None) => { /* user cancelled — no-op */ }
            Err(e) => save_message.set(Some(fl!(
                "editor-insert-image-error",
                reason = e.to_string()
            ))),
        }
    });
}
