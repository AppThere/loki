// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! View-facing types for [`super::document_view`]: the view mode, cursor and
//! selection positions, tile context, and [`DocumentViewProps`]. Extracted to
//! keep `document_view.rs` under the file-size ceiling.

use std::sync::Arc;

use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_layout::PaginatedLayout;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    /// Fixed print layout — one fixed-size page tile per page (needs the GPU
    /// paint path). This is the default on large viewports.
    #[default]
    Paginated,
    /// Reflowable, web-page-style continuous layout that wraps to the viewport
    /// width. The default on small viewports, and the only mode available on
    /// the Android CPU path.
    Reflow,
}

// ── RendererCursorPos ─────────────────────────────────────────────────────────

/// Minimal cursor position for GPU painting. No Loro dependency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RendererCursorPos {
    pub page_index: usize,
    pub paragraph_index: usize,
    pub byte_offset: usize,
}

/// Caret + optional range selection for GPU painting. `anchor == focus` (by
/// paragraph/byte) means a collapsed caret with no selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RendererSelection {
    pub focus: RendererCursorPos,
    pub anchor: RendererCursorPos,
}

impl RendererSelection {
    /// True when there is no range selection (anchor and focus coincide).
    pub fn is_collapsed(&self) -> bool {
        self.anchor.paragraph_index == self.focus.paragraph_index
            && self.anchor.byte_offset == self.focus.byte_offset
    }
}

/// A right-click on a page tile, carrying both the tile-local layout-point
/// coordinates (for an accurate hit test) and the window-relative client
/// coordinates (to anchor a floating menu at the cursor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileContext {
    /// Index of the page tile that was right-clicked.
    pub page_index: usize,
    /// X within the tile, in layout points (from `element_coordinates`).
    pub x_pt: f32,
    /// Y within the tile, in layout points.
    pub y_pt: f32,
    /// Window-relative X of the cursor, in CSS pixels.
    pub client_x: f32,
    /// Window-relative Y of the cursor, in CSS pixels.
    pub client_y: f32,
}

// ── DocumentViewProps ─────────────────────────────────────────────────────────

/// Props for the DocumentView component.
#[derive(Props, Clone)]
pub struct DocumentViewProps {
    pub doc: Arc<Document>,
    /// Paginated layout already computed by the editor for `doc`, reused in
    /// paginated mode so the renderer does not lay the document out a second
    /// time (the single canonical layout). `None` falls back to computing it
    /// on the render path (e.g. before the first editor layout, or on the
    /// Android CPU reflow path).
    pub paginated_layout: Option<Arc<PaginatedLayout>>,
    pub viewport_height_px: f64,
    /// Current vertical scroll offset of the editor's scroll container, in CSS
    /// px. Drives tile virtualization: only pages within ~one screen of this
    /// offset are GPU-rendered. The editor owns the scroll container (this
    /// component is laid out inside it), so the scroll position must be passed
    /// in — the renderer's own scroll signal is not updated by the real scroll.
    pub viewport_top_px: f64,
    /// The caret / selection focus position.
    pub cursor_pos: Option<RendererCursorPos>,
    /// The selection anchor. When it differs from `cursor_pos`, a range
    /// selection is highlighted between them (reflow mode).
    pub selection_anchor: Option<RendererCursorPos>,
    /// Current layout mode. Ignored on the Android CPU path, which only supports
    /// [`ViewMode::Reflow`].
    pub view_mode: ViewMode,
    /// Available viewport width in CSS pixels for [`ViewMode::Reflow`].
    /// `<= 0` means "not yet measured" — the view falls back to paginated
    /// rendering until a real width arrives.
    pub reflow_width_px: f64,
    /// Paginated render zoom factor (1.0 = 100%). Scales the page tiles' CSS
    /// size and paint transform together; the layout (in points) and reflow
    /// mode are unaffected.
    #[props(default = 1.0)]
    pub zoom: f64,
    /// Called with `(page_index, x_pt, y_pt)` in layout points when the user
    /// clicks a page tile in **paginated** mode. The caller performs the hit test
    /// and updates cursor state.
    pub on_tile_click: EventHandler<(usize, f32, f32)>,
    /// Called with `(block_index, byte_offset)` when the user clicks in
    /// **reflow** mode. This component owns the reflow layout, so it hit-tests
    /// the click itself and reports the resolved document position.
    pub on_reflow_click: EventHandler<(usize, usize)>,
    /// Called with `(block_index, byte_offset)` while drag-selecting in
    /// **reflow** mode (mouse moved with a button held). The caller extends the
    /// selection focus to this position.
    pub on_reflow_drag: EventHandler<(usize, usize)>,
    /// Called when a page tile is right-clicked (paginated mode), carrying the
    /// accurate tile-local + client coordinates. Drives the spelling context menu.
    pub on_tile_context: EventHandler<TileContext>,
    /// Vertical gap between page tiles in paginated mode, in CSS px. Injected by
    /// the app (from `appthere_ui::tokens::PAGE_GAP_PX`) rather than imported
    /// here, so the render layer does not depend on the UI layer — Spec 01 audit
    /// A-8. Zeroed automatically in reflow mode.
    pub page_gap_px: f64,
    /// Bottom padding below the last page, in CSS px (from
    /// `appthere_ui::tokens::SPACE_6`). Injected for the same reason as
    /// `page_gap_px`.
    pub content_padding_bottom_px: f32,
}

impl PartialEq for DocumentViewProps {
    fn eq(&self, _other: &Self) -> bool {
        false // Conservatively always re-render
    }
}
