// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Editor screen route component.
//!
//! Implements the document editor shell.  The layout is a vertical flex column:
//!
//! ```text
//! ┌─────────────────────────┐
//! │      Top Toolbar        │  flex-shrink: 0
//! ├─────────────────────────┤
//! │                         │
//! │  ┌───────────────────┐  │  height: calc(100vh - chrome), overflow-y: auto
//! │  │   Page 1          │  │
//! │  └───────────────────┘  │
//! │  ┌───────────────────┐  │
//! │  │   Page 2          │  │
//! │  └───────────────────┘  │
//! │         …               │
//! │                         │
//! ├─────────────────────────┤
//! │     Bottom Toolbar      │  flex-shrink: 0
//! └─────────────────────────┘
//! ```
//!
//! The `path` route parameter carries a serialised
//! [`loki_file_access::FileAccessToken`].  Document loading runs in a
//! [`use_resource`] async task so the shell renders immediately while the
//! import pipeline runs in the background.

// Pipeline entry points (confirmed from source):
// loki_file_access: FileAccessToken::deserialize(s: &str) -> Result<FileAccessToken, TokenParseError>
//                   token.open_read() -> Result<Box<dyn ReadSeek>, AccessError>
//                   where ReadSeek: std::io::Read + std::io::Seek + Send
// loki_ooxml:       DocxImport::import(reader: impl Read + Seek, options: DocxImportOptions)
//                       -> Result<loki_doc_model::Document, OoxmlError>
//                   (via loki_doc_model::io::DocumentImport trait)
// loki_vello:       paint_layout(scene: &mut vello::Scene, layout: &DocumentLayout,
//                       font_cache: &mut FontDataCache, offset: (f32, f32), scale: f32,
//                       page_index: Option<usize>)
//                   (called inside WgpuSurface — see components/wgpu_surface.rs)

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_doc_model::loro_bridge::derive_loro_cursor;
use loki_doc_model::loro_mutation::{delete_text, get_paragraph_text, insert_text};
use loki_file_access::FileAccessToken;
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
use loki_layout::LayoutOptions;
use loki_theme::tokens;

use crate::components::document_source::DocumentState;
use crate::components::toolbar::{BottomToolbar, TopToolbar};
use crate::components::wgpu_surface::WgpuSurface;
use crate::editing::cursor::{prev_grapheme_boundary, next_grapheme_boundary, CursorState, DocumentPosition};
use crate::editing::hit_test::hit_test_document;

#[derive(Clone, PartialEq, Copy)]
pub enum EditorMode { Reading, Editing }
use crate::error::LoadError;
use crate::utils::display_title_from_path;

/// Document editor shell component.
///
/// Receives the `path` route parameter (a serialised
/// [`loki_file_access::FileAccessToken`]) and renders the three-panel editor
/// layout: top toolbar, scrollable page canvas area, and bottom status bar.
///
/// Document loading runs asynchronously via [`use_resource`]:
/// - **Loading** — toolbars are shown immediately; canvas shows "Opening
///   document\u{2026}" placeholder via [`WgpuSurface`].
/// - **Error** — inline error message with a "Go back" button; no panic.
/// - **Loaded** — document is passed to [`WgpuSurface`] for scene building.
#[component]
pub fn Editor(path: String) -> Element {
    let title = display_title_from_path(&path);

    let editor_mode = use_signal(|| EditorMode::Reading);
    let mut loro_doc: Signal<Option<loro::LoroDoc>> = use_signal(|| None);

    // ── Shared document state (Strategy A) ───────────────────────────────────
    // Created here so editor mouse handlers can read paginated_layout from the
    // same Arc<Mutex<DocumentState>> that LokiDocumentSource writes to after
    // each layout rebuild.  Passed down to WgpuSurface as a prop.
    let doc_state: Arc<Mutex<DocumentState>> = use_hook(|| {
        Arc::new(Mutex::new(DocumentState {
            document: None,
            generation: 0,
            page_count: 0,
            canvas_width: 0.0,
            visible_rect: None,
            page_width_px: tokens::PAGE_WIDTH_PX,
            page_height_px: tokens::PAGE_HEIGHT_PX,
            cursor_state: None,
            paginated_layout: None,
        }))
    });

    // ── Cursor / selection state ──────────────────────────────────────────────
    let mut cursor_state: Signal<CursorState> = use_signal(CursorState::new);
    let mut is_dragging: Signal<bool> = use_signal(|| false);

    // Canvas origin in window-relative CSS pixels (Strategy C — calculated).
    //
    // canvas_origin.y is exact: TOOLBAR_HEIGHT_TOP + SPACE_6 (top padding of
    // the scroll container).
    //
    // canvas_origin.x depends on the window inner width, which Blitz/Dioxus
    // native does not yet expose to Dioxus components (no window-size hook).
    // A default of 1280 px is assumed; update `window_width` when a resize
    // API becomes available.
    //
    // TODO(window-size): subscribe to window resize events and update
    // `window_width` once Blitz exposes inner_size to Dioxus components.
    let window_width: Signal<f32> = use_signal(|| 1280.0_f32);
    // scroll_offset is always 0.0: Blitz does not expose node.scroll_offset
    // to Dioxus components (see wgpu_surface.rs TODO(partial-render)).
    let scroll_offset: Signal<f32> = use_signal(|| 0.0_f32);

    // Kick off the document-loading pipeline.  The future is async but all
    // I/O is synchronous under the hood; a spawn_blocking wrapper would be
    // appropriate for large files once the executor supports it.
    let document_load: Resource<Result<Document, LoadError>> = {
        let path = path.clone();
        use_resource(move || {
            let path = path.clone();
            async move { load_document(path) }
        })
    };

    let navigator = use_navigator();
    
    use_effect(move || {
        if let Some(Ok(doc)) = &*document_load.value().read_unchecked() {
            if loro_doc().is_none() {
                match loki_doc_model::loro_bridge::document_to_loro(doc) {
                    Ok(l_doc) => loro_doc.set(Some(l_doc)),
                    Err(e) => tracing::warn!("Failed to initialize Loro sync bridge: {}", e),
                }
            }
        }
    });

    let layout_opts = match editor_mode() {
        EditorMode::Reading  => LayoutOptions::default(),
        EditorMode::Editing  => LayoutOptions { preserve_for_editing: true },
    };

    // ── Canvas origin (Strategy C) ────────────────────────────────────────────
    // The canvas is flex-centered within the full-width scroll container.
    // canvas_origin.x = (window_width - page_width_px) / 2
    // canvas_origin.y = TOOLBAR_HEIGHT_TOP + SPACE_6  (top padding of scroll container)
    //
    // page_width_px is approximated with the A4 default; a loaded document will
    // typically match this or be close enough for the MVP.  A future improvement
    // can read the actual page dimensions from the layout result.
    let canvas_origin_y = tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6;
    let canvas_origin_x = (window_width() - tokens::PAGE_WIDTH_PX) / 2.0;
    let canvas_origin = (canvas_origin_x, canvas_origin_y);
    let page_width_px = tokens::PAGE_WIDTH_PX;
    let page_height_px = tokens::PAGE_HEIGHT_PX;
    let page_gap_px = tokens::PAGE_GAP_PX;

    // ── Scroll container height ───────────────────────────────────────────────
    //
    // Blitz scroll event path (blitz-shell-0.2.3/src/window.rs:388):
    //   WindowEvent::MouseWheel
    //     → scroll_node_by_has_changed(hover_node_id)   [document.rs:1258]
    //     → bubbles DOM tree until a node with can_y_scroll=true is found
    //     → updates node.scroll_offset when scroll_height() > 0
    //     → blitz-paint applies offset as a translate    [render.rs:235-245]
    //
    // Scrollability (blitz-dom-0.2.4/src/document.rs:1272-1277):
    //   can_y_scroll = overflow_y ∈ {Scroll, Auto}
    //   Both values map to taffy::Overflow::Scroll in
    //   stylo_taffy-0.2.0/src/convert.rs:227-228.
    //
    // scroll_height() = max(0, content_size.height − size.height)
    //   (taffy-0.9.2/src/tree/layout.rs:339-344)
    //   For scroll to engage, content (pages) must exceed the container's
    //   computed height.
    //
    // ROOT CAUSE of prior non-functional scroll:
    //   Using `flex: 1` left the scroll container's taffy height indefinite
    //   when Blitz failed to propagate the `height: 100vh` definite size from
    //   the root div through the flex chain.  With an indefinite height taffy
    //   expands the container to fit all children → content_size == size →
    //   scroll_height() == 0 → scroll_node_by_has_changed returns false.
    //
    // FIX: explicit `height: calc(100vh - Npx)` gives taffy a concrete
    //   Dimension::Length(n), bypassing the flex chain entirely.  The pages
    //   overflow the finite box → scroll_height > 0 → scrolling works.
    //
    // KNOWN LIMITATION: no public API in dioxus-native-0.7.4 exposes
    //   node.scroll_offset back to Dioxus components, so visible_rect stays
    //   None.  onwheel handlers never fire; scroll is driven entirely by
    //   blitz-shell's WindowEvent::MouseWheel handler.
    //   TODO(partial-render): wire scroll_offset → visible_rect once Blitz
    //   exposes a scroll-position hook to Dioxus components.
    //
    // ── Toolbar hit-test overlap when scrolled ────────────────────────────────
    //
    // BUG (Blitz): Node::hit() [blitz-dom-0.2.4/src/node/node.rs:716-773]
    //   adjusts incoming coordinates by +scroll_offset before checking bounds:
    //     adjusted_y = pointer_y - container_y + scroll_offset_y
    //     matches_self = !(adjusted_y < 0 || …)
    //   When scroll_offset_y ≥ container_y (= TOOLBAR_HEIGHT_TOP), adjusted_y
    //   is always ≥ 0 for any pointer_y ≥ 0, so the scroll container claims
    //   every click including those in the toolbar row.
    //
    //   pointer-events:none is NOT implemented in this Blitz version
    //   (blitz-dom-0.2.4/src/node/node.rs has no reference to PointerEvents).
    //
    // FIX: paint_children are sorted ascending by z_index
    //   [blitz-dom-0.2.4/src/layout/damage.rs:353-383].  Hit testing iterates
    //   paint_children in REVERSE, so the highest z_index wins.  TopToolbar
    //   carries `position: relative; z-index: 10` (see toolbar.rs); the scroll
    //   container has the default z_index: 0.  After sorting, TopToolbar is the
    //   last entry in paint_children and therefore the FIRST tested — it
    //   captures clicks in its own (correct) bounds before the scroll container
    //   is tried.
    let chrome_px =
        tokens::TOOLBAR_HEIGHT_TOP as u32 + tokens::TOOLBAR_HEIGHT_BOTTOM as u32;

    // Pre-clone the Arc so each move closure owns an independent reference.
    let doc_state_down = Arc::clone(&doc_state);
    let doc_state_move = Arc::clone(&doc_state);
    let doc_state_kbd = Arc::clone(&doc_state);

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex: 1; \
                 background: {bg}; font-family: system-ui, sans-serif;",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // ── Top toolbar (flex-shrink: 0) ───────────────────────────────────
            TopToolbar {
                title: title,
                editor_mode: editor_mode
            }

            // ── Scroll container ──────────────────────────────────────────────
            div {
                style: format!(
                    "height: calc(100vh - {chrome_px}px); min-height: 0; \
                     overflow-y: auto; background: {bg}; padding: {p}px 0;",
                    bg = tokens::COLOR_SURFACE_BASE,
                    p  = tokens::SPACE_6,
                ),
                // tabindex makes the scroll container focusable so onkeydown fires.
                tabindex: "0",

                // ── Keyboard handler for text insertion / deletion ─────────────
                //
                // Guarded on EditorMode::Editing; no-op in read-only mode.
                // Relies on the LoroDoc having been initialised by the use_effect
                // below (document_to_loro).  Mutations update the LoroDoc then
                // re-derive the Document snapshot and bump doc_state.generation so
                // that LokiDocumentSource::render() picks up the change on the
                // next frame.
                //
                // MVP limitation: single-section documents only.
                // paragraph_index from DocumentPosition is used directly as the
                // block_index within section 0.
                onkeydown: move |evt| {
                    if editor_mode() != EditorMode::Editing { return; }

                    let Some(focus) = cursor_state.read().focus.clone() else { return; };
                    // MVP: single-section assumption — paragraph_index == block_index.
                    let block_idx = focus.paragraph_index;

                    match evt.key() {
                        Key::Character(s) if !s.is_empty() => {
                            let new_pos = {
                                let guard = loro_doc.read();
                                let Some(ldoc) = guard.as_ref() else { return; };
                                if let Err(e) = insert_text(ldoc, 0, block_idx, focus.byte_offset, &s) {
                                    tracing::warn!("insert_text failed: {e}");
                                    return;
                                }
                                match loki_doc_model::loro_bridge::loro_to_document(ldoc) {
                                    Ok(new_doc) => {
                                        if let Ok(mut state) = doc_state_kbd.lock() {
                                            state.document = Some(new_doc);
                                            state.generation = state.generation.wrapping_add(1);
                                        }
                                    }
                                    Err(e) => tracing::warn!("loro_to_document failed: {e}"),
                                }
                                DocumentPosition {
                                    page_index: focus.page_index,
                                    paragraph_index: focus.paragraph_index,
                                    byte_offset: focus.byte_offset + s.len(),
                                }
                            };
                            cursor_state.write().anchor = Some(new_pos.clone());
                            cursor_state.write().focus = Some(new_pos);
                        }
                        Key::Backspace => {
                            if focus.byte_offset == 0 { return; }
                            let new_pos = {
                                let guard = loro_doc.read();
                                let Some(ldoc) = guard.as_ref() else { return; };
                                let Ok(text) = get_paragraph_text(ldoc, 0, block_idx) else { return; };
                                let prev = prev_grapheme_boundary(&text, focus.byte_offset);
                                let del_len = focus.byte_offset - prev;
                                if let Err(e) = delete_text(ldoc, 0, block_idx, prev, del_len) {
                                    tracing::warn!("delete_text (backspace) failed: {e}");
                                    return;
                                }
                                match loki_doc_model::loro_bridge::loro_to_document(ldoc) {
                                    Ok(new_doc) => {
                                        if let Ok(mut state) = doc_state_kbd.lock() {
                                            state.document = Some(new_doc);
                                            state.generation = state.generation.wrapping_add(1);
                                        }
                                    }
                                    Err(e) => tracing::warn!("loro_to_document failed: {e}"),
                                }
                                DocumentPosition {
                                    page_index: focus.page_index,
                                    paragraph_index: focus.paragraph_index,
                                    byte_offset: prev,
                                }
                            };
                            cursor_state.write().anchor = Some(new_pos.clone());
                            cursor_state.write().focus = Some(new_pos);
                        }
                        Key::Delete => {
                            let guard = loro_doc.read();
                            let Some(ldoc) = guard.as_ref() else { return; };
                            let Ok(text) = get_paragraph_text(ldoc, 0, block_idx) else { return; };
                            if focus.byte_offset >= text.len() { return; }
                            let next = next_grapheme_boundary(&text, focus.byte_offset);
                            let del_len = next - focus.byte_offset;
                            if let Err(e) = delete_text(ldoc, 0, block_idx, focus.byte_offset, del_len) {
                                tracing::warn!("delete_text (delete-fwd) failed: {e}");
                                return;
                            }
                            match loki_doc_model::loro_bridge::loro_to_document(ldoc) {
                                Ok(new_doc) => {
                                    if let Ok(mut state) = doc_state_kbd.lock() {
                                        state.document = Some(new_doc);
                                        state.generation = state.generation.wrapping_add(1);
                                    }
                                }
                                Err(e) => tracing::warn!("loro_to_document failed: {e}"),
                            }
                            // Cursor stays at the same byte_offset.
                        }
                        _ => {}
                    }
                },

                // ── Pointer event handlers for cursor / selection ──────────────
                //
                // All three handlers guard on EditorMode::Editing so no cursor
                // state is modified in read-only mode.  client_coordinates()
                // provides window-relative CSS logical pixels (Strategy C).

                onmousedown: move |evt| {
                    if editor_mode() != EditorMode::Editing { return; }
                    is_dragging.set(true);

                    let layout = doc_state_down.lock().ok()
                        .and_then(|s| s.paginated_layout.clone());
                    let Some(layout) = layout else { return; };

                    let coords = evt.client_coordinates();
                    let pos = hit_test_document(
                        coords.x as f32,
                        coords.y as f32,
                        canvas_origin,
                        scroll_offset(),
                        &layout,
                        page_width_px,
                        page_height_px,
                        page_gap_px,
                    );

                    if let Some(p) = pos {
                        let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                            derive_loro_cursor(
                                ldoc,
                                p.page_index,
                                p.paragraph_index,
                                p.byte_offset,
                            )
                        });
                        let mut cs = cursor_state.write();
                        cs.loro_cursor = loro_cursor;
                        cs.anchor = Some(p.clone());
                        cs.focus = Some(p);
                    }
                },

                onmousemove: move |evt| {
                    if !is_dragging() || editor_mode() != EditorMode::Editing { return; }

                    let layout = doc_state_move.lock().ok()
                        .and_then(|s| s.paginated_layout.clone());
                    let Some(layout) = layout else { return; };

                    let coords = evt.client_coordinates();
                    let pos = hit_test_document(
                        coords.x as f32,
                        coords.y as f32,
                        canvas_origin,
                        scroll_offset(),
                        &layout,
                        page_width_px,
                        page_height_px,
                        page_gap_px,
                    );
                    if let Some(p) = pos {
                        cursor_state.write().focus = Some(p);
                    }
                },

                onmouseup: move |_| {
                    is_dragging.set(false);
                },

                match &*document_load.value().read_unchecked() {
                    // Resource is still running — show placeholder via WgpuSurface.
                    None => rsx! {
                        WgpuSurface {
                            document: None,
                            layout_opts: layout_opts.clone(),
                            visible_rect: None,
                            cursor_state: None,
                            doc_state: Arc::clone(&doc_state),
                        }
                    },

                    // Import pipeline failed.
                    Some(Err(e)) => {
                        let msg = e.to_string();
                        rsx! {
                            div {
                                style: format!(
                                    "display: flex; flex-direction: column; \
                                     justify-content: center; align-items: center; \
                                     gap: {gap}px;",
                                    gap = tokens::SPACE_4,
                                ),
                                span {
                                    style: format!(
                                        "font-size: {size}px; color: {fg};",
                                        size = tokens::FONT_SIZE_BODY,
                                        fg   = tokens::COLOR_TEXT_PRIMARY,
                                    ),
                                    "Could not open document: {msg}"
                                }
                                button {
                                    style: format!(
                                        "padding: {p}px {p2}px; background: {bg}; \
                                         border: 1px solid {border}; border-radius: 4px; \
                                         font-size: {size}px; cursor: pointer;",
                                        p      = tokens::SPACE_2,
                                        p2     = tokens::SPACE_4,
                                        bg     = tokens::COLOR_SURFACE_PAGE,
                                        border = tokens::COLOR_BORDER_DEFAULT,
                                        size   = tokens::FONT_SIZE_BODY,
                                    ),
                                    onclick: move |_| { navigator.push(crate::routes::Route::Home {}); },
                                    "Go back"
                                }
                            }
                        }
                    },

                    Some(Ok(doc)) => {
                        let cs = if editor_mode() == EditorMode::Editing {
                            Some(cursor_state.read().clone())
                        } else {
                            None
                        };
                        rsx! {
                            WgpuSurface {
                                document: Some(doc.clone()),
                                layout_opts: layout_opts.clone(),
                                visible_rect: None,
                                cursor_state: cs,
                                doc_state: Arc::clone(&doc_state),
                            }
                        }
                    },
                }
            }

            // ── Bottom status bar (flex-shrink: 0) ────────────────────────────
            BottomToolbar {
                page_info: "Page 1 of 1".to_string(),
                zoom_info:  "100%".to_string(),
            }
        }
    }
}

// ── Loading pipeline ──────────────────────────────────────────────────────────

/// Detected document format, derived from the file extension in the token's
/// display name.
enum DocumentFormat {
    Docx,
    Odt,
    Unsupported(String),
}

/// Inspect the display name on `token` and return the [`DocumentFormat`] for
/// this file.  The extension comparison is case-insensitive.
fn detect_format(token: &FileAccessToken) -> DocumentFormat {
    match token
        .display_name()
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("docx") => DocumentFormat::Docx,
        Some("odt") => DocumentFormat::Odt,
        Some(ext) => DocumentFormat::Unsupported(ext.to_string()),
        None => DocumentFormat::Unsupported(String::new()),
    }
}

/// Deserialise `path` → detect format → open file → import → return
/// [`Document`].
///
/// Format is determined from the file extension in the
/// [`FileAccessToken`] display name before the file is opened, so the
/// reader is only consumed once.  All I/O is synchronous; this function is
/// called inside an `async move` block in [`use_resource`] so that loading
/// does not block the initial render of the editor shell.
fn load_document(path: String) -> Result<Document, LoadError> {
    let token = FileAccessToken::deserialize(&path)?;
    let format = detect_format(&token);
    let reader = token.open_read()?;
    let doc = match format {
        DocumentFormat::Docx => {
            DocxImport::import(reader, DocxImportOptions::default())
                .map_err(LoadError::Ooxml)?
            // TODO(odt-fidelity): DOCX rendering gaps (styles, page size) tracked separately.
        }
        DocumentFormat::Odt => {
            OdtImport::import(reader, OdtImportOptions::default())
                .map_err(LoadError::Odt)?
            // TODO(odt-fidelity): ODT rendering gaps — some paragraph styles, list
            // indents, and image placement may not render correctly yet.
        }
        DocumentFormat::Unsupported(ext) => {
            return Err(LoadError::UnsupportedFormat(ext));
        }
    };
    Ok(doc)
}
