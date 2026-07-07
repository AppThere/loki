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

/// Lucide `link` — two interlocking chain links (two subpaths in one `d`).
pub const LUCIDE_LINK: &str =
    "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71";

/// Lucide `image` — framed picture with a sun and a mountain (subpaths in one
/// `d`; the circle is approximated with two arc halves).
pub const LUCIDE_IMAGE: &str =
    "M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2zM10 9a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0zM21 15l-5-5L5 21";

/// Lucide `table` — a bordered grid (outer frame plus one mid horizontal and
/// one mid vertical divider). Used for Insert → Table.
pub const LUCIDE_TABLE: &str =
    "M12 3v18M3 9h18M3 15h18M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z";

/// Lucide `superscript`-style reference mark reused for Insert → Footnote — a
/// down-step baseline with a raised reference tick, evoking a footnote marker.
pub const LUCIDE_FOOTNOTE: &str = "M4 5h6M4 5v10a3 3 0 0 0 6 0M16 5v6m0-6h4m-4 0-1 1";

/// Lucide `more-horizontal` — three dots. Used for the ribbon overflow ("More")
/// menu button. Rendered as three round-capped zero-length strokes (Lucide's own
/// dot idiom), so it needs `stroke-linecap: round` (which [`AtIcon`] sets).
pub const LUCIDE_MORE_HORIZONTAL: &str = "M5 12h.01M12 12h.01M19 12h.01";

/// Lucide `trash-2` — a waste bin with lid and two vertical bars. Used for the
/// Table contextual tab's Delete Table action.
pub const LUCIDE_TRASH_2: &str =
    "M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M10 11v6M14 11v6";

// App-custom table-op glyphs (not Lucide): a box for the affected row/column
// plus a `+` / `−`, drawn in the same 24×24 stroked style as the Lucide set.

/// Insert row below: a horizontal bar with a plus beneath it.
pub const AT_TABLE_ROW_INSERT: &str = "M4 4h16v6h-16zM12 14v6M9 17h6";

/// Insert row above: a horizontal bar with a plus above it.
pub const AT_TABLE_ROW_INSERT_ABOVE: &str = "M4 14h16v6h-16zM12 4v6M9 7h6";

/// Delete row: a horizontal bar with a minus below it.
pub const AT_TABLE_ROW_DELETE: &str = "M4 4h16v6h-16zM9 17h6";

/// Insert column right: a vertical bar with a plus to its right.
pub const AT_TABLE_COL_INSERT: &str = "M4 4h6v16h-6zM17 9v6M14 12h6";

/// Insert column left: a vertical bar with a plus to its left.
pub const AT_TABLE_COL_INSERT_LEFT: &str = "M14 4h6v16h-6zM7 9v6M4 12h6";

/// Delete column: a vertical bar with a minus to its right.
pub const AT_TABLE_COL_DELETE: &str = "M4 4h6v16h-6zM14 12h6";

// App-custom glyphs (not Lucide): an "A" beside an up/down arrow, for the
// grow/shrink font-size buttons.

/// Increase font size: an "A" with an upward arrow.
pub const AT_FONT_GROW: &str = "M6 15 10 7 14 15M7.5 12h5M18 8v7M15.5 10.5 18 8 20.5 10.5";

/// Decrease font size: an "A" with a downward arrow.
pub const AT_FONT_SHRINK: &str = "M6 15 10 7 14 15M7.5 12h5M18 8v7M15.5 12.5 18 15 20.5 12.5";

// App-custom page-orientation glyphs (not Lucide): a tall vs. wide page rect.

/// Portrait orientation: a tall page rectangle.
pub const AT_PAGE_PORTRAIT: &str = "M7 3h10v18H7z";

/// Landscape orientation: a wide page rectangle.
pub const AT_PAGE_LANDSCAPE: &str = "M3 7h18v10H3z";

// App-custom margin-preset glyphs (not Lucide): a page rectangle with an inner
// content rectangle whose inset shows the margin size. Disambiguated by tooltip.

/// Normal margins: a page with a moderate inset content area.
pub const AT_MARGIN_NORMAL: &str = "M5 3h14v18H5zM8 6h8v12H8z";

/// Narrow margins: a page with a small inset (large content area).
pub const AT_MARGIN_NARROW: &str = "M5 3h14v18H5zM6.5 4.5h11v15h-11z";

/// Wide margins: a page with a wide horizontal inset (narrow content area).
pub const AT_MARGIN_WIDE: &str = "M5 3h14v18H5zM9 6h6v12H9z";

// App-custom page-size glyphs (not Lucide): page rectangles of the paper's
// aspect ratio. Disambiguated by tooltip.

/// A4 paper: a tall, narrow page (≈1:1.41).
pub const AT_PAGE_A4: &str = "M7 3h10v18H7z";

/// US Letter paper: a slightly wider, shorter page (≈1:1.29).
pub const AT_PAGE_LETTER: &str = "M6 4h12v16H6z";

// App-custom column-count glyphs (not Lucide): a page with N-1 vertical
// divider lines.

/// One column: a plain page.
pub const AT_COLUMNS_ONE: &str = "M5 4h14v16H5z";

/// Two columns: a page split by one vertical divider.
pub const AT_COLUMNS_TWO: &str = "M5 4h14v16H5zM12 4v16";

/// Three columns: a page split by two vertical dividers.
pub const AT_COLUMNS_THREE: &str = "M5 4h14v16H5zM9.7 4v16M14.3 4v16";

// App-custom References-tab glyphs (not Lucide).

/// Insert table of contents: stacked outline entries of varying, indented width.
pub const AT_TOC_INSERT: &str = "M4 5h16M4 10h10M8 15h12M4 20h9";

/// Update table of contents: a clockwise refresh arrow (regenerate the field).
pub const AT_TOC_UPDATE: &str = "M20 11A8 8 0 1 0 18 16M20 5v6h-6";

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
