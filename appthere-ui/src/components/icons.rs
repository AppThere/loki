// SPDX-License-Identifier: Apache-2.0

//! Lucide icon helper — renders SVG icons from Lucide path data.
//!
//! # COMPAT(dioxus-native)
//!
//! SVG element rendering via Blitz is unconfirmed.  All ribbon buttons retain
//! an `aria_label` prop for screen-reader accessibility regardless of whether
//! the SVG is displayed.  If SVG does not render, text labels in `aria_label`
//! remain as the accessible name.

use dioxus::prelude::*;

// ── Lucide path data constants ────────────────────────────────────────────────

/// Lucide `undo-2` — single path.
pub const LUCIDE_UNDO: &str =
    "M9 14 4 9l5-5M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5v0a5.5 5.5 0 0 1-5.5 5.5H11";

/// Lucide `redo-2` — single path.
pub const LUCIDE_REDO: &str =
    "M15 14l5-5-5-5M19 9H8.5A5.5 5.5 0 0 0 3 14.5v0A5.5 5.5 0 0 0 8.5 20H13";

/// Lucide `bold` — single path.
pub const LUCIDE_BOLD: &str =
    "M6 12h9a4 4 0 0 1 0 8H7a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1h7a4 4 0 0 1 0 8";

/// Lucide `italic` — three subpaths in one `d` string.
pub const LUCIDE_ITALIC: &str = "M19 4h-9M14 20H5M15 4 9 20";

/// Lucide `underline` — two subpaths in one `d` string.
pub const LUCIDE_UNDERLINE: &str = "M6 4v6a6 6 0 0 0 12 0V4M4 20h16";

/// Lucide `strikethrough` — three subpaths.
pub const LUCIDE_STRIKETHROUGH: &str = "M16 4H9a3 3 0 0 0-2.83 4M14 12a4 4 0 0 1 0 8H6M4 12h16";

/// Lucide `superscript` — two subpaths.
pub const LUCIDE_SUPERSCRIPT: &str = "M4 19l8-8M12 19l-8-8m14.5-9.5V9h-4l4-4.5";

/// Lucide `subscript` — two subpaths.
pub const LUCIDE_SUBSCRIPT: &str = "M4 5l8 8M12 5l-8 8m14.5 6.5V19h-4l4-4.5";

/// Lucide `save` — floppy-disk outline.
pub const LUCIDE_SAVE: &str =
    "M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2zM17 21v-8H7v8M7 3v5h8";

/// Lucide `download` — tray with a downward arrow. Used for "Save As" to
/// visually distinguish it from the plain floppy-disk Save.
pub const LUCIDE_DOWNLOAD: &str = "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3";

/// Lucide `layout-template` — a wide bar above two smaller panes. Used for
/// "Save as Template" to distinguish it from the plain Save / Save As actions.
pub const LUCIDE_LAYOUT_TEMPLATE: &str = "M3 3h18v7H3zM3 14h9v7H3zM16 14h5v7h-5z";

/// Lucide `align-left` — three lines, all left-aligned.
pub const LUCIDE_ALIGN_LEFT: &str = "M15 12H3M17 6H3M13 18H3";

/// Lucide `align-center` — three lines, centred.
pub const LUCIDE_ALIGN_CENTER: &str = "M17 12H7M21 6H3M19 18H5";

/// Lucide `align-right` — three lines, all right-aligned.
pub const LUCIDE_ALIGN_RIGHT: &str = "M21 12H9M21 6H7M21 18H11";

/// Lucide `align-justify` — three equal-width lines.
pub const LUCIDE_ALIGN_JUSTIFY: &str = "M3 6h18M3 12h18M3 18h18";

/// Lucide `pilcrow` — paragraph mark (¶).
pub const LUCIDE_PILCROW: &str = "M13 4v16M17 4v16M8 4h4a4 4 0 0 1 0 8H8";

// ── AtIcon component ──────────────────────────────────────────────────────────

/// Renders a single Lucide SVG icon.
///
/// Each Lucide icon uses a 24×24 viewBox, `stroke="currentColor"`, `fill="none"`,
/// `stroke-width="2"`, rounded linecap and linejoin.  The `size` prop controls
/// the CSS width and height in logical pixels.
///
/// # COMPAT(dioxus-native)
///
/// SVG rendering in Blitz is unconfirmed.  Callers should also supply an
/// `aria_label` on the parent button for screen-reader accessibility.
///
/// # Touch target
///
/// This component renders an icon only; the parent button is responsible for
/// the 44×44 px minimum touch target (WCAG 2.5.8).
#[component]
pub fn AtIcon(
    /// Lucide `d` path attribute — may encode multiple sub-paths (M…M…).
    path_d: String,
    /// Width and height of the icon in logical pixels.
    #[props(default = 18.0)]
    size: f32,
) -> Element {
    // COMPAT(dioxus-native): SVG rendering in Blitz is unconfirmed.
    rsx! {
        svg {
            "viewBox": "0 0 24 24",
            "width": "{size}",
            "height": "{size}",
            "stroke": "currentColor",
            "stroke-width": "2",
            "fill": "none",
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            path { "d": "{path_d}" }
        }
    }
}
