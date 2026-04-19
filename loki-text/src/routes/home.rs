// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Home screen route component.
//!
//! Renders a mobile-first three-section layout:
//! 1. Header bar with the app name and a settings stub.
//! 2. Template gallery — horizontal-scroll row of template cards.
//! 3. Recent files — vertically stacked file entries.
//! 4. Open File button — invokes the platform file picker and navigates to the
//!    editor on selection.
//!
//! On viewports wider than [`loki_theme::tokens::BREAKPOINT_DESKTOP_PX`],
//! sections 2 and 3 are displayed side-by-side in a two-column layout.

use dioxus::prelude::*;
use loki_file_access::{FilePicker, PickOptions};
use loki_theme::tokens;

use crate::routes::Route;

// ── Placeholder data ──────────────────────────────────────────────────────────

/// A single template card entry (static placeholder).
struct TemplateEntry {
    title: &'static str,
    /// CSS hex colour for the type-swatch thumbnail.
    swatch: &'static str,
}

const TEMPLATES: &[TemplateEntry] = &[
    TemplateEntry { title: "Blank",   swatch: "#BDBDBD" },
    TemplateEntry { title: "Letter",  swatch: "#42A5F5" },
    TemplateEntry { title: "Report",  swatch: "#66BB6A" },
    TemplateEntry { title: "Resume",  swatch: "#FFA726" },
    TemplateEntry { title: "Invoice", swatch: "#AB47BC" },
];

/// A single recent-file entry (static placeholder).
struct RecentFile {
    /// Filename including extension.
    name: &'static str,
    /// Truncated display path shown beneath the filename.
    path: &'static str,
    /// Human-readable last-modified timestamp.
    modified: &'static str,
}

const RECENT_FILES: &[RecentFile] = &[
    RecentFile {
        name:     "Q1 Report.docx",
        path:     "~/Documents/Work/2026/…",
        modified: "2026-04-12  14:30",
    },
    RecentFile {
        name:     "Meeting Notes.odt",
        path:     "~/Documents/Meetings/…",
        modified: "2026-04-11  09:15",
    },
    RecentFile {
        name:     "Budget Draft.docx",
        path:     "~/Documents/Finance/…",
        modified: "2026-04-09  16:45",
    },
];

// ── Component ─────────────────────────────────────────────────────────────────

/// Home screen component.
///
/// Entry point after the app launches.  Lets the user open an existing document
/// or create a new one from a template.
///
/// # Responsive layout
///
/// * **Mobile (< [`tokens::BREAKPOINT_DESKTOP_PX`] px):** single-column;
///   template gallery scrolls horizontally above the recent-files list.
/// * **Desktop (≥ [`tokens::BREAKPOINT_DESKTOP_PX`] px):** template gallery
///   and recent-files list are displayed side-by-side.
///
/// # Viewport width signal
///
/// `viewport_width` is initialised to a mobile default (`375.0`).  Wire it to
/// the actual window dimension via the Dioxus Native window-resize API once
/// that API stabilises.
#[component]
pub fn Home() -> Element {
    // ── Signals ───────────────────────────────────────────────────────────────

    // Holds the last file-picker error message, if any.
    let pick_error: Signal<Option<String>> = use_signal(|| None);

    // Hover state for the primary "Open File" button.
    let mut open_btn_hovered = use_signal(|| false);

    // Current viewport width in CSS pixels.
    // Defaults to a mobile width; update via a window-resize event when the
    // Dioxus Native resize hook is available.
    let viewport_width = use_signal(|| 375.0_f32);
    let is_desktop = viewport_width() >= tokens::BREAKPOINT_DESKTOP_PX;

    // ── Navigation ───────────────────────────────────────────────────────────
    let navigator = use_navigator();

    // ── Event handlers ───────────────────────────────────────────────────────

    // Opens the platform file-picker dialog.
    // On success, serialises the FileAccessToken and navigates to the editor
    // route.  On cancellation, does nothing.  On error, writes the message
    // into `pick_error` for inline display.
    let on_open_file = move |_| {
        let nav = navigator.clone();
        let mut err_sig = pick_error;
        spawn(async move {
            let picker = FilePicker::new();
            let opts = PickOptions {
                mime_types: vec![
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                        .to_string(),
                    "application/vnd.oasis.opendocument.text".to_string(),
                ],
                filter_label: Some("Documents".to_string()),
                multi: false,
            };
            match picker.pick_file_to_open(opts).await {
                Ok(Some(token)) => {
                    nav.push(Route::Editor { path: token.serialize() });
                }
                Ok(None) => { /* user cancelled — no-op */ }
                Err(e) => {
                    *err_sig.write() = Some(e.to_string());
                }
            }
        });
    };

    // ── Derived styles ────────────────────────────────────────────────────────

    let body_style = if is_desktop {
        format!(
            "display: flex; flex-direction: row; gap: {gap}px; \
             padding: {pad}px; flex: 1; overflow: hidden; min-height: 0;",
            gap = tokens::SPACE_4,
            pad = tokens::SPACE_4,
        )
    } else {
        format!(
            "display: flex; flex-direction: column; \
             padding: {pad}px; flex: 1; overflow-y: auto;",
            pad = tokens::SPACE_4,
        )
    };

    let open_btn_bg = if open_btn_hovered() {
        tokens::COLOR_ACCENT_PRIMARY_HOVER
    } else {
        tokens::COLOR_ACCENT_PRIMARY
    };

    let open_btn_width = if is_desktop {
        format!("{}px", tokens::BUTTON_WIDTH_DESKTOP_MAX)
    } else {
        "100%".to_string()
    };

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex: 1; \
                 background: {bg}; font-family: system-ui, sans-serif; \
                 color: {fg};",
                bg = tokens::COLOR_SURFACE_BASE,
                fg = tokens::COLOR_TEXT_PRIMARY,
            ),

            // ── Header bar ────────────────────────────────────────────────────
            div {
                style: format!(
                    "height: {h}px; background: {bg}; \
                     border-bottom: 1px solid {border}; \
                     display: flex; align-items: center; \
                     padding: 0 {pad}px; flex-shrink: 0;",
                    h      = tokens::TOOLBAR_HEIGHT_TOP,
                    bg     = tokens::COLOR_SURFACE_PAGE,
                    border = tokens::COLOR_BORDER_DEFAULT,
                    pad    = tokens::SPACE_4,
                ),
                span {
                    style: format!(
                        "font-size: {size}px; font-weight: 700; \
                         color: {fg}; flex: 1;",
                        size = tokens::FONT_SIZE_HEADING,
                        fg   = tokens::COLOR_TEXT_PRIMARY,
                    ),
                    "Loki"
                }
                // Settings icon — stub (non-functional)
                button {
                    style: format!(
                        "background: transparent; border: none; \
                         color: {fg}; font-size: {size}px; \
                         cursor: pointer; padding: {pad}px;",
                        fg   = tokens::COLOR_TEXT_SECONDARY,
                        size = tokens::FONT_SIZE_HEADING,
                        pad  = tokens::SPACE_2,
                    ),
                    "⚙"
                }
            }

            // ── Body: template gallery + recent files ─────────────────────────
            div {
                style: body_style,

                // Template gallery section
                div {
                    style: if is_desktop {
                        "flex: 1; min-width: 0;".to_string()
                    } else {
                        format!("margin-bottom: {mb}px;", mb = tokens::SPACE_6)
                    },
                    h2 {
                        style: format!(
                            "font-size: {size}px; color: {fg}; \
                             margin: 0 0 {mb}px 0; font-weight: 600;",
                            size = tokens::FONT_SIZE_BODY,
                            fg   = tokens::COLOR_TEXT_SECONDARY,
                            mb   = tokens::SPACE_2,
                        ),
                        "Templates"
                    }
                    div {
                        style: format!(
                            "display: flex; flex-direction: row; \
                             gap: {gap}px; overflow-x: auto; \
                             padding-bottom: {pb}px;",
                            gap = tokens::SPACE_3,
                            pb  = tokens::SPACE_2,
                        ),
                        for entry in TEMPLATES {
                            div {
                                key: "{entry.title}",
                                style: format!(
                                    "flex-shrink: 0; width: 100px; \
                                     background: {bg}; border-radius: 4px; \
                                     padding: {pad}px; \
                                     display: flex; flex-direction: column; \
                                     align-items: center; gap: {gap}px;",
                                    bg  = tokens::COLOR_SURFACE_PAGE,
                                    pad = tokens::SPACE_3,
                                    gap = tokens::SPACE_2,
                                ),
                                // Coloured swatch representing the template type
                                div {
                                    style: format!(
                                        "width: 60px; height: 80px; \
                                         background: {bg}; border-radius: 2px;",
                                        bg = entry.swatch,
                                    ),
                                }
                                span {
                                    style: format!(
                                        "font-size: {size}px; \
                                         color: {fg}; text-align: center;",
                                        size = tokens::FONT_SIZE_LABEL,
                                        fg   = tokens::COLOR_TEXT_PRIMARY,
                                    ),
                                    "{entry.title}"
                                }
                            }
                        }
                    }
                }

                // Recent files section
                div {
                    style: if is_desktop {
                        "flex: 1; min-width: 0; overflow-y: auto;".to_string()
                    } else {
                        String::new()
                    },
                    h2 {
                        style: format!(
                            "font-size: {size}px; color: {fg}; \
                             margin: 0 0 {mb}px 0; font-weight: 600;",
                            size = tokens::FONT_SIZE_BODY,
                            fg   = tokens::COLOR_TEXT_SECONDARY,
                            mb   = tokens::SPACE_2,
                        ),
                        "Recent"
                    }
                    div {
                        style: format!(
                            "display: flex; flex-direction: column; \
                             gap: {gap}px;",
                            gap = tokens::SPACE_2,
                        ),
                        for file in RECENT_FILES {
                            div {
                                key: "{file.name}",
                                style: format!(
                                    "background: {bg}; border-radius: 4px; \
                                     padding: {pv}px {ph}px; \
                                     display: flex; flex-direction: column; \
                                     gap: {gap}px;",
                                    bg  = tokens::COLOR_SURFACE_PAGE,
                                    pv  = tokens::SPACE_3,
                                    ph  = tokens::SPACE_4,
                                    gap = tokens::SPACE_1,
                                ),
                                span {
                                    style: format!(
                                        "font-size: {size}px; font-weight: 600; \
                                         color: {fg};",
                                        size = tokens::FONT_SIZE_BODY,
                                        fg   = tokens::COLOR_TEXT_PRIMARY,
                                    ),
                                    "{file.name}"
                                }
                                span {
                                    style: format!(
                                        "font-size: {size}px; color: {fg};",
                                        size = tokens::FONT_SIZE_LABEL,
                                        fg   = tokens::COLOR_TEXT_SECONDARY,
                                    ),
                                    "{file.path}"
                                }
                                span {
                                    style: format!(
                                        "font-size: {size}px; color: {fg};",
                                        size = tokens::FONT_SIZE_LABEL,
                                        fg   = tokens::COLOR_TEXT_SECONDARY,
                                    ),
                                    "{file.modified}"
                                }
                            }
                        }
                    }
                }
            }

            // ── Inline error banner ────────────────────────────────────────────
            if let Some(err) = pick_error() {
                div {
                    style: format!(
                        "background: {bg}; border: 1px solid {border}; \
                         margin: {m}px; padding: {p}px; border-radius: 4px; \
                         color: {fg}; font-size: {size}px;",
                        bg     = tokens::COLOR_STATUS_ERROR_BG,
                        border = tokens::COLOR_STATUS_ERROR_BORDER,
                        m      = tokens::SPACE_4,
                        p      = tokens::SPACE_3,
                        fg     = tokens::COLOR_STATUS_ERROR_TEXT,
                        size   = tokens::FONT_SIZE_LABEL,
                    ),
                    "Could not open file picker: {err}"
                }
            }

            // ── Open File button ───────────────────────────────────────────────
            div {
                style: format!(
                    "padding: {p}px; flex-shrink: 0;",
                    p = tokens::SPACE_4,
                ),
                button {
                    style: format!(
                        "width: {w}; display: block; margin: 0 auto; \
                         background: {bg}; color: {fg}; \
                         border: none; border-radius: 4px; \
                         height: 48px; font-size: {size}px; \
                         font-weight: 600; cursor: pointer;",
                        w    = open_btn_width,
                        bg   = open_btn_bg,
                        fg   = tokens::COLOR_SURFACE_PAGE,
                        size = tokens::FONT_SIZE_BODY,
                    ),
                    onmouseenter: move |_| { open_btn_hovered.set(true); },
                    onmouseleave: move |_| { open_btn_hovered.set(false); },
                    onclick:      on_open_file,
                    "Open File…"
                }
            }
        }
    }
}
