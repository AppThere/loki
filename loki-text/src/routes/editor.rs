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
use loki_doc_model::loro_bridge::{derive_loro_cursor, document_to_loro};
use loki_doc_model::loro_mutation::{delete_text, get_block_text, insert_text};
use loki_file_access::FileAccessToken;
use loki_layout::LayoutOptions;
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
use loki_theme::tokens;

use crate::components::document_source::{apply_mutation_and_relayout, DocumentState};
use crate::components::toolbar::{BottomToolbar, TopToolbar};
use crate::components::wgpu_surface::WgpuSurface;
use crate::editing::cursor::{
    next_grapheme_boundary, prev_grapheme_boundary, CursorState, DocumentPosition,
};
use crate::editing::hit_test::hit_test_document;
use crate::error::LoadError;
use crate::utils::display_title_from_path;

/// Editor view mode toggle.
#[derive(Clone, PartialEq, Copy)]
pub enum EditorMode {
    Reading,
    Editing,
}

/// Document editor shell component.
///
/// Receives the `path` route parameter (a serialised
/// [`loki_file_access::FileAccessToken`]) and renders the three-panel editor
/// layout: top toolbar, scrollable page canvas area, and bottom status bar.
///
/// Document loading runs asynchronously via [`use_resource`]:
/// - **Loading** — toolbars are shown immediately; canvas shows "Opening
///   document…" placeholder via [`WgpuSurface`].
/// - **Error** — inline error message with a "Go back" button; no panic.
/// - **Loaded** — document is passed to [`WgpuSurface`] for scene building.
#[component]
pub fn Editor(path: String) -> Element {
    let title = display_title_from_path(&path);

    let editor_mode = use_signal(|| EditorMode::Reading);
    let mut loro_doc: Signal<Option<loro::LoroDoc>> = use_signal(|| None);

    // ── Shared document state ─────────────────────────────────────────────────
    //
    // Created here (not inside WgpuSurface) so that mouse and keyboard handlers
    // can close over the Arc and read `paginated_layout` for hit-testing, or
    // bump `generation` after a Loro mutation to trigger a GPU re-render.
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
            preserve_for_editing: false,
        }))
    });

    // Pre-clone the Arc once so closures below can each capture their own clone.
    let doc_state_mousemove = Arc::clone(&doc_state);
    let doc_state_keys = Arc::clone(&doc_state);
    let doc_state_prop = Arc::clone(&doc_state);

    // ── Cursor / selection state ──────────────────────────────────────────────
    let mut cursor_state: Signal<CursorState> = use_signal(CursorState::new);
    // `is_dragging`: mouse button is currently held down.
    let mut is_dragging: Signal<bool> = use_signal(|| false);
    // Client-coordinate position of the most recent mousedown.  Used to gate
    // focus updates in `onmousemove` behind a drag threshold so that tiny
    // cursor jitter during a click does not create a spurious text selection.
    let mut drag_origin: Signal<Option<(f32, f32)>> = use_signal(|| None);

    // window_width and scroll_offset are used only by the onmousemove drag
    // selection handler via hit_test_document.  Click placement (onmousedown)
    // now uses element_coordinates() from the patched dioxus-native-dom and
    // no longer needs these values.
    //
    // window_width: window inner width in CSS px for centering the hit-test
    // origin.  Still defaults to 1280 because Blitz does not expose a
    // window-resize hook to Dioxus components.
    // TODO(window-size): update once Blitz exposes inner_size.
    //
    // scroll_offset: always 0.0 because Blitz does not route scroll events
    // through Dioxus; drag selection will be imprecise after scrolling.
    // TODO(partial-render): wire when Blitz exposes node.scroll_offset.
    let window_width: Signal<f32> = use_signal(|| 1280.0_f32);
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
                match document_to_loro(doc) {
                    Ok(l_doc) => loro_doc.set(Some(l_doc)),
                    Err(e) => tracing::warn!("Failed to initialize Loro sync bridge: {}", e),
                }
            }
        }
    });

    let layout_opts = match editor_mode() {
        EditorMode::Reading => LayoutOptions::default(),
        EditorMode::Editing => LayoutOptions { preserve_for_editing: true },
    };

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

                // tabindex="0" is required for this div to receive keyboard
                // focus (and therefore onkeydown events) in Blitz/Dioxus.
                tabindex: "0",

                // ── Pointer event handlers for cursor / selection ──────────────
                //
                // All three handlers guard on EditorMode::Editing so no cursor
                // state is modified in read-only mode.  client_coordinates()
                // provides window-relative CSS logical pixels.
                //
                // onmousedown on the scroll container fires via bubbling from
                // child canvas elements.  It records the raw client position so
                // that onmousemove can apply a drag threshold before extending
                // the selection — preventing cursor jitter during a plain click
                // from creating a spurious text selection.
                onmousedown: move |evt| {
                    let c = evt.client_coordinates();
                    drag_origin.set(Some((c.x as f32, c.y as f32)));
                },

                onmousemove: {
                    let doc_state = Arc::clone(&doc_state_mousemove);
                    move |evt| {
                        if !is_dragging() || editor_mode() != EditorMode::Editing {
                            return;
                        }

                        // Only extend the selection once the pointer has moved
                        // beyond DRAG_THRESHOLD_PX from the mousedown origin.
                        // This prevents tiny cursor jitter during a click from
                        // creating a spurious selection via hit_test_document.
                        const DRAG_THRESHOLD_SQ: f32 = 4.0 * 4.0; // 4 CSS px
                        let coords = evt.client_coordinates();
                        let cx = coords.x as f32;
                        let cy = coords.y as f32;
                        if let Some((ox, oy)) = drag_origin() {
                            let dx = cx - ox;
                            let dy = cy - oy;
                            if dx * dx + dy * dy < DRAG_THRESHOLD_SQ {
                                return;
                            }
                        }

                        // Read layout and page dimensions from shared state.
                        let (layout_opt, page_width_px, page_height_px) = {
                            let Ok(state) = doc_state.lock() else { return; };
                            (
                                state.paginated_layout.clone(),
                                state.page_width_px,
                                state.page_height_px,
                            )
                        };

                        let Some(layout) = layout_opt else { return; };

                        let x_off = (window_width() - page_width_px).max(0.0) / 2.0;
                        let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);

                        let pos = hit_test_document(
                            cx,
                            cy,
                            origin,
                            scroll_offset(),
                            &layout,
                            page_width_px,
                            page_height_px,
                            page_gap_px,
                        );

                        // During drag: update focus only, anchor stays fixed.
                        if let Some(p) = pos {
                            cursor_state.write().focus = Some(p);
                        }
                    }
                },

                onmouseup: move |_| {
                    is_dragging.set(false);
                    drag_origin.set(None);
                },

                // ── Keyboard input ─────────────────────────────────────────────
                //
                // Requires tabindex="0" on the scroll container (above) to
                // receive focus.  Guards on EditorMode::Editing so keys in
                // read-only mode are ignored.
                onkeydown: {
                    let doc_state = Arc::clone(&doc_state_keys);
                    let loro_doc = loro_doc.clone();
                    move |evt| {
                        tracing::info!("Editor: onkeydown fired: {:?}", evt.key());
                        if editor_mode() != EditorMode::Editing {
                            return;
                        }

                        let focus = cursor_state.read().focus.clone();
                        let Some(focus) = focus else { return; };

                        match evt.key() {
                            // ── Printable characters ───────────────────────────
                            Key::Character(ref ch) => {
                                // Ctrl/Meta key combinations are formatting
                                // shortcuts — deferred to Session 4.
                                if evt.modifiers().ctrl() || evt.modifiers().meta() {
                                    return;
                                }
                                let ch = ch.clone();

                                // Insert into Loro.
                                {
                                    let ldoc_guard = loro_doc.read();
                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                    if insert_text(ldoc, focus.paragraph_index, focus.byte_offset, &ch).is_err() {
                                        return;
                                    }
                                }

                                // Re-layout and update shared state.
                                {
                                    let ldoc_guard = loro_doc.read();
                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                    apply_mutation_and_relayout(&doc_state, ldoc);
                                }

                                // Advance cursor by the inserted string length.
                                let new_offset = focus.byte_offset + ch.len();
                                let new_pos = DocumentPosition {
                                    byte_offset: new_offset,
                                    ..focus
                                };
                                let mut cs = cursor_state.write();
                                cs.focus = Some(new_pos.clone());
                                cs.anchor = Some(new_pos);
                            }

                            // ── Backspace ──────────────────────────────────────
                            Key::Backspace => {
                                if focus.byte_offset == 0 {
                                    // TODO(3b): merge with previous paragraph
                                    return;
                                }

                                let text = {
                                    let ldoc_guard = loro_doc.read();
                                    ldoc_guard
                                        .as_ref()
                                        .map(|l| get_block_text(l, focus.paragraph_index))
                                        .unwrap_or_default()
                                };
                                let prev = prev_grapheme_boundary(&text, focus.byte_offset);
                                let len = focus.byte_offset - prev;

                                {
                                    let ldoc_guard = loro_doc.read();
                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                    if delete_text(ldoc, focus.paragraph_index, prev, len).is_err() {
                                        return;
                                    }
                                }

                                {
                                    let ldoc_guard = loro_doc.read();
                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                    apply_mutation_and_relayout(&doc_state, ldoc);
                                }

                                let new_pos = DocumentPosition {
                                    byte_offset: prev,
                                    ..focus
                                };
                                let mut cs = cursor_state.write();
                                cs.focus = Some(new_pos.clone());
                                cs.anchor = Some(new_pos);
                            }

                            // ── Forward delete ─────────────────────────────────
                            Key::Delete => {
                                let text = {
                                    let ldoc_guard = loro_doc.read();
                                    ldoc_guard
                                        .as_ref()
                                        .map(|l| get_block_text(l, focus.paragraph_index))
                                        .unwrap_or_default()
                                };
                                if focus.byte_offset >= text.len() {
                                    return;
                                }
                                let next = next_grapheme_boundary(&text, focus.byte_offset);
                                let len = next - focus.byte_offset;

                                {
                                    let ldoc_guard = loro_doc.read();
                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                    if delete_text(
                                        ldoc,
                                        focus.paragraph_index,
                                        focus.byte_offset,
                                        len,
                                    )
                                    .is_err()
                                    {
                                        return;
                                    }
                                }

                                {
                                    let ldoc_guard = loro_doc.read();
                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                    apply_mutation_and_relayout(&doc_state, ldoc);
                                }
                                // Cursor stays at the same offset after forward delete.
                            }

                            // ── Navigation — deferred to Session 3b ───────────
                            Key::ArrowLeft
                            | Key::ArrowRight
                            | Key::ArrowUp
                            | Key::ArrowDown
                            | Key::Home
                            | Key::End => {
                                // TODO(3b): arrow key navigation
                            }

                            Key::Enter => {
                                // TODO(3b): paragraph split
                            }

                            _ => {}
                        }
                    }
                },

                match &*document_load.value().read_unchecked() {
                    // Resource is still running — show placeholder via WgpuSurface.
                    None => rsx! {
                        WgpuSurface {
                            doc_state: Arc::clone(&doc_state_prop),
                            document: None,
                            layout_opts: layout_opts.clone(),
                            visible_rect: None,
                            cursor_state: None,
                            on_mousedown: |_| {},
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
                                doc_state: Arc::clone(&doc_state_prop),
                                document: Some(doc.clone()),
                                layout_opts: layout_opts.clone(),
                                visible_rect: None,
                                cursor_state: cs,
                                on_mousedown: move |p: DocumentPosition| {
                                    if editor_mode() != EditorMode::Editing {
                                        return;
                                    }
                                    is_dragging.set(true);
                                    
                                    let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                                        derive_loro_cursor(ldoc, p.paragraph_index, p.byte_offset)
                                    });
                                    let mut cs = cursor_state.write();
                                    cs.loro_cursor = loro_cursor;
                                    cs.anchor = Some(p.clone());
                                    cs.focus = Some(p);
                                }
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
