// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Right-click spelling-menu resolution for the document canvas.
//!
//! Extracted from `editor_canvas` (Spec 08 Phase 1): that file is over the
//! 300-line ceiling and pinned by the CI ratchet, so mounting the caret-follow
//! sensor there had to be paid for by taking something out. This handler body
//! is the natural candidate — it is already a free function, it is the only
//! part of the module that is about spelling rather than about the canvas, and
//! it has no shared state with the rest of the file.

use std::sync::Arc;

use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;
use loki_renderer::TileContext;

use super::editor_spell::{SpellMenu, resolve_spell_menu};
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::{hit_test::hit_test_page, state::DocumentState};

/// Right-click handler body: resolves the word under the tile-local coordinates
/// in `ctx` (accurate, via `element_coordinates` — no window-centring math),
/// selects it, and opens the spelling menu anchored at the cursor. A no-op when
/// there is no word at the point.
pub(super) fn open_spell_panel_at(
    ctx: TileContext,
    doc_state: &Arc<std::sync::Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    service: &SpellService,
    mut cursor_state: Signal<CursorState>,
    mut spell_menu: Signal<Option<SpellMenu>>,
) {
    let layout_opt = {
        let Ok(s) = doc_state.lock() else { return };
        s.paginated_layout.clone()
    };
    let Some(layout) = layout_opt else { return };
    let Some(pos) = hit_test_page(ctx.page_index, ctx.x_pt, ctx.y_pt, &layout) else {
        return;
    };
    match resolve_spell_menu(loro_doc, service, pos.paragraph_index, pos.byte_offset) {
        Some(mut menu) => {
            // Anchor the floating menu at the cursor (window-relative coords).
            // NOT viewport-relative, despite the name: this stack returns the
            // DOM's `pageX/pageY` from `client_coordinates()` and leaves
            // `page_coordinates()` unimplemented — the two are swapped. See
            // "Documented stack deviations" in docs/patches.md.
            //
            // Window-relative plus *top-level* scroll, which is always zero here
            // (the app root is 100vh / overflow: hidden). Inner container scroll
            // is excluded, which is harmless only while the containing block is
            // the anchor's scroll parent — TODO(t4.1-popover): no longer true
            // once this is root-hosted.
            menu.anchor_x = ctx.client_x;
            menu.anchor_y = ctx.client_y;
            // Select the whole word so the user sees what the suggestions apply to.
            let word_pos = |byte_offset| {
                DocumentPosition::top_level(pos.page_index, menu.paragraph_index, byte_offset)
            };
            cursor_state.write().anchor = Some(word_pos(menu.byte_start));
            cursor_state.write().focus = Some(word_pos(menu.byte_end));
            spell_menu.set(Some(menu));
        }
        // No word at the point — just place the caret.
        None => {
            cursor_state.write().anchor = Some(pos.clone());
            cursor_state.write().focus = Some(pos);
        }
    }
}
