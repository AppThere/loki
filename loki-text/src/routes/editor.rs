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
//! │    WGPU Canvas          │  flex: 1, overflow: hidden
//! │    (loki-vello)         │
//! │                         │
//! ├─────────────────────────┤
//! │     Bottom Toolbar      │  flex-shrink: 0
//! └─────────────────────────┘
//! ```
//!
//! The `path` route parameter carries a serialised
//! [`loki_file_access::FileAccessToken`].  Actual document loading is out of
//! scope for this session — the token is passed through to [`WgpuSurface`] as
//! a stub.

use dioxus::prelude::*;

use loki_theme::tokens;

use crate::components::toolbar::{BottomToolbar, TopToolbar};
use crate::components::wgpu_surface::WgpuSurface;
use crate::utils::display_title_from_path;

/// Document editor shell component.
///
/// Receives the `path` route parameter (a serialised
/// [`loki_file_access::FileAccessToken`]) and renders the three-panel editor
/// layout: top toolbar, WGPU canvas area, and bottom status bar.
///
/// # Out of scope
///
/// * Document parsing / loading (deferred to a future session).
/// * Text editing, selection, or cursor logic.
/// * Scroll-linked partial rendering (the seam is present in [`WgpuSurface`];
///   the implementation is not).
#[component]
pub fn Editor(path: String) -> Element {
    let title = display_title_from_path(&path);

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex: 1; \
                 background: {bg}; font-family: system-ui, sans-serif;",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // ── Top toolbar (flex-shrink: 0) ───────────────────────────────────
            TopToolbar { title }

            // ── WGPU canvas area (flex: 1) ─────────────────────────────────────
            WgpuSurface {
                document_path: Some(path),
                visible_rect: None,
            }

            // ── Bottom status bar (flex-shrink: 0) ────────────────────────────
            BottomToolbar {
                page_info: "Page 1 of 1".to_string(),
                zoom_info:  "100%".to_string(),
            }
        }
    }
}

