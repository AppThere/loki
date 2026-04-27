// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Toolbar primitives for the editor shell.
//!
//! Provides two components:
//! * [`TopToolbar`] — fixed-height bar at the top of the editor with navigation,
//!   title, and action stubs.
//! * [`BottomToolbar`] — fixed-height status bar at the bottom with page/zoom
//!   information and a mode-toggle placeholder.
//!
//! Both components use `flex-shrink: 0` to maintain a constant height inside
//! the editor's vertical flex layout.  Neither uses `position: fixed`, which
//! Blitz does not support.
//!
//! ## TopToolbar z-index
//!
//! `TopToolbar` sets `position: relative; z-index: 10` on its root div.  This
//! is required to beat the scroll container in Blitz hit testing.
//!
//! Root cause: `blitz-dom-0.2.4/src/node/node.rs Node::hit()` adjusts the
//! incoming y-coordinate by `+scroll_offset.y`, then uses `y < 0` as the
//! upper-boundary guard.  Once `scroll_offset.y ≥ TOOLBAR_HEIGHT_TOP` that
//! guard is never triggered for any click in the viewport, so the scroll
//! container incorrectly claims hits in the toolbar row.
//!
//! Mitigation: `paint_children` are sorted ascending by (z_index,
//! position_to_order) in `blitz-dom-0.2.4/src/layout/damage.rs:353-383`.
//! Hit testing reverses that list, so the element with the *highest* z_index
//! is tested first.  `z-index: 10` on the toolbar (vs. the default 0 on the
//! scroll container) ensures the toolbar always wins when the cursor is in its
//! row, regardless of how far the document has been scrolled.

use dioxus::prelude::*;
use loki_theme::tokens;

use crate::routes::Route;
use crate::routes::editor::EditorMode;

// ── TopToolbar ────────────────────────────────────────────────────────────────

/// Top editor toolbar.
///
/// Contains:
/// * A **back/close button** that navigates to [`Route::Home`].
/// * A **document title** display in the center.
/// * **Save** and **Share** icon stubs (non-functional placeholders).
///
/// # Layout
///
/// Uses `flex-shrink: 0` so the bar never collapses inside the editor's
/// `flex-direction: column` container.
#[component]
pub fn TopToolbar(title: String, mut editor_mode: Signal<EditorMode>) -> Element {
    let navigator = use_navigator();

    let mut save_hovered  = use_signal(|| false);
    let mut share_hovered = use_signal(|| false);
    let mut back_hovered  = use_signal(|| false);

    let back_bg = if back_hovered() {
        tokens::COLOR_SURFACE_BASE
    } else {
        "transparent"
    };

    let save_bg = if save_hovered() {
        tokens::COLOR_SURFACE_BASE
    } else {
        "transparent"
    };

    let share_bg = if share_hovered() {
        tokens::COLOR_SURFACE_BASE
    } else {
        "transparent"
    };

    rsx! {
        div {
            style: format!(
                // position:relative + z-index:10 — required for correct hit
                // testing when the scroll container is scrolled; see module
                // doc for the full explanation.
                "position: relative; z-index: 10; \
                 height: {h}px; background: {bg}; \
                 border-bottom: 1px solid {border}; \
                 display: flex; align-items: center; \
                 padding: 0 {pad}px; flex-shrink: 0; gap: {gap}px;",
                h      = tokens::TOOLBAR_HEIGHT_TOP,
                bg     = tokens::COLOR_SURFACE_PAGE,
                border = tokens::COLOR_BORDER_DEFAULT,
                pad    = tokens::SPACE_2,
                gap    = tokens::SPACE_2,
            ),

            // Back / close button
            button {
                style: format!(
                    "background: {bg}; border: none; border-radius: 4px; \
                     color: {fg}; font-size: {size}px; cursor: pointer; \
                     padding: {p}px {p2}px; flex-shrink: 0;",
                    bg   = back_bg,
                    fg   = tokens::COLOR_TEXT_PRIMARY,
                    size = tokens::FONT_SIZE_BODY,
                    p    = tokens::SPACE_1,
                    p2   = tokens::SPACE_2,
                ),
                onmouseenter: move |_| { back_hovered.set(true); },
                onmouseleave: move |_| { back_hovered.set(false); },
                onclick: move |_| { navigator.push(Route::Home {}); },
                "← Back"
            }

            // Document title (centered, flex: 1)
            span {
                style: format!(
                    "flex: 1; font-size: {size}px; font-weight: 600; \
                     color: {fg}; text-align: center; \
                     overflow: hidden;",
                    size = tokens::FONT_SIZE_BODY,
                    fg   = tokens::COLOR_TEXT_PRIMARY,
                ),
                "{title}"
            }

            // Save icon stub
            button {
                style: format!(
                    "background: {bg}; border: none; border-radius: 4px; \
                     color: {fg}; font-size: {size}px; cursor: pointer; \
                     padding: {p}px; flex-shrink: 0;",
                    bg   = save_bg,
                    fg   = tokens::COLOR_TEXT_SECONDARY,
                    size = tokens::FONT_SIZE_HEADING,
                    p    = tokens::SPACE_2,
                ),
                onmouseenter: move |_| { save_hovered.set(true); },
                onmouseleave: move |_| { save_hovered.set(false); },
                // Non-functional stub — document saving is out of scope.
                "💾"
            }

            // Share icon stub
            button {
                style: format!(
                    "background: {bg}; border: none; border-radius: 4px; \
                     color: {fg}; font-size: {size}px; cursor: pointer; \
                     padding: {p}px; flex-shrink: 0;",
                    bg   = share_bg,
                    fg   = tokens::COLOR_TEXT_SECONDARY,
                    size = tokens::FONT_SIZE_HEADING,
                    p    = tokens::SPACE_2,
                ),
                onmouseenter: move |_| { share_hovered.set(true); },
                onmouseleave: move |_| { share_hovered.set(false); },
                // Non-functional stub — sharing is out of scope.
                "↗"
            }

            // Mode toggle button
            button {
                style: format!(
                    "background: {bg}; border: 1px solid {border}; border-radius: 4px; \
                     color: {fg}; font-size: {size}px; font-weight: 500; cursor: pointer; \
                     padding: {p}px {p2}px; flex-shrink: 0; margin-left: {m}px;",
                    bg   = if editor_mode() == EditorMode::Editing { tokens::COLOR_SURFACE_BASE } else { tokens::COLOR_SURFACE_PAGE },
                    border = tokens::COLOR_BORDER_DEFAULT,
                    fg   = tokens::COLOR_TEXT_PRIMARY,
                    size = tokens::FONT_SIZE_BODY,
                    p    = tokens::SPACE_1,
                    p2   = tokens::SPACE_2,
                    m    = tokens::SPACE_2,
                ),
                onclick: move |_| { 
                    editor_mode.set(if editor_mode() == EditorMode::Reading {
                        EditorMode::Editing
                    } else {
                        EditorMode::Reading
                    });
                },
                if editor_mode() == EditorMode::Reading { "Editor Mode" } else { "Read Mode" }
            }
        }
    }
}

// ── BottomToolbar ─────────────────────────────────────────────────────────────

/// Bottom editor status bar.
///
/// Displays:
/// * **Page count** indicator (e.g., `"Page 1 of 1"`).
/// * **Zoom level** display (e.g., `"100%"`).
/// * A placeholder region on the right for future editing-mode toggles.
///
/// Uses `flex-shrink: 0` to keep a constant height in the editor flex column.
#[component]
pub fn BottomToolbar(
    /// Page indicator string, e.g. `"Page 1 of 1"`.
    page_info: String,
    /// Zoom level string, e.g. `"100%"`.
    zoom_info: String,
) -> Element {
    rsx! {
        div {
            style: format!(
                "height: {h}px; background: {bg}; \
                 border-top: 1px solid {border}; \
                 display: flex; align-items: center; \
                 padding: 0 {pad}px; flex-shrink: 0; gap: {gap}px;",
                h      = tokens::TOOLBAR_HEIGHT_BOTTOM,
                bg     = tokens::COLOR_SURFACE_PAGE,
                border = tokens::COLOR_BORDER_DEFAULT,
                pad    = tokens::SPACE_4,
                gap    = tokens::SPACE_4,
            ),

            // Page count indicator
            span {
                style: format!(
                    "font-size: {size}px; color: {fg};",
                    size = tokens::FONT_SIZE_LABEL,
                    fg   = tokens::COLOR_TEXT_SECONDARY,
                ),
                "{page_info}"
            }

            // Zoom level display
            span {
                style: format!(
                    "font-size: {size}px; color: {fg};",
                    size = tokens::FONT_SIZE_LABEL,
                    fg   = tokens::COLOR_TEXT_SECONDARY,
                ),
                "Zoom: {zoom_info}"
            }

            // Spacer — pushes mode-toggle placeholder to the right
            div { style: "flex: 1;" }

            // Mode-toggle placeholder
            span {
                style: format!(
                    "font-size: {size}px; color: {fg};",
                    size = tokens::FONT_SIZE_LABEL,
                    fg   = tokens::COLOR_BORDER_DEFAULT,
                ),
                // Non-functional stub — editing mode toggles are out of scope.
                "[ mode ]"
            }
        }
    }
}
