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

use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_file_access::FileAccessToken;
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
use loki_theme::tokens;

use crate::components::toolbar::{BottomToolbar, TopToolbar};
use crate::components::wgpu_surface::WgpuSurface;
use loki_layout::LayoutOptions;

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

    let mut editor_mode = use_signal(|| EditorMode::Reading);
    let mut loro_doc: Signal<Option<loro::LoroDoc>> = use_signal(|| None);

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

                match &*document_load.value().read_unchecked() {
                    // Resource is still running — show placeholder via WgpuSurface.
                    None => rsx! {
                        WgpuSurface { 
                            document: None, 
                            layout_opts: layout_opts.clone(),
                            visible_rect: None 
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

                    Some(Ok(doc)) => rsx! {
                        WgpuSurface {
                            document: Some(doc.clone()),
                            layout_opts: layout_opts.clone(),
                            visible_rect: None,
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
