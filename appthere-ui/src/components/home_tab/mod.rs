// SPDX-License-Identifier: Apache-2.0

//! `AtHomeTab` — home tab content surface (template gallery + recent documents).
//!
//! Composed from two private sub-components:
//! * [`template_gallery::AtTemplateGallery`] — horizontal card scroll row.
//! * [`recent_files::AtRecentFileList`] — vertical recent document list.
//!
//! All user-visible strings are accepted as [`String`] props so translated
//! strings from `loki_i18n::fl!()` can be passed directly.

mod recent_files;
mod recent_row;
mod template_gallery;

use dioxus::prelude::*;
use loki_i18n::fl;
use recent_files::AtRecentFileList;
use template_gallery::AtTemplateGallery;

use crate::responsive::use_breakpoint;
use crate::safe_area::use_safe_area;
use crate::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_ACCENT_PRIMARY_HOVER, COLOR_STATUS_ERROR_BG,
    COLOR_STATUS_ERROR_BORDER, COLOR_STATUS_ERROR_TEXT, COLOR_SURFACE_BASE, COLOR_TEXT_ON_CHROME,
    COLOR_TEXT_ON_CHROME_SECONDARY, COLOR_TEXT_PRIMARY,
};
use crate::tokens::layout::TAB_BAR_HEIGHT;
use crate::tokens::spacing::{RADIUS_SM, SPACE_2, SPACE_3, SPACE_4, SPACE_6, TOUCH_MIN};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD,
};

// ── Public types ──────────────────────────────────────────────────────────────

/// Describes a single built-in document template shown in the gallery.
#[derive(Clone, PartialEq, Debug)]
pub struct BuiltinTemplate {
    /// Displayed template name, e.g. `"Blank"`, `"Letter"`.
    pub name: String,
    /// One-line description shown in a tooltip (deferred — see COMPAT note).
    /// Retained in the data model for future tooltip/detail-panel use.
    pub description: String,
    /// Short format label on the card thumbnail, e.g. `"DOTX"`, `"OTT"`.
    pub format_label: String,
}

/// Describes a recently opened document entry.
#[derive(Clone, PartialEq, Debug)]
pub struct RecentDocument {
    /// Document title (filename stem).
    pub title: String,
    /// Truncated display path shown beneath the title.
    pub path: String,
    /// Pre-formatted last-modified timestamp string.
    /// i18n formatting is the caller's responsibility; this component
    /// displays the string as-is.
    pub modified_at: String,
}

// ── AtHomeTab ─────────────────────────────────────────────────────────────────

/// Home tab content surface.
///
/// Renders a header, a template gallery, and a recent documents list.
/// At the Compact size class (`use_breakpoint`, < 600 px) the gallery and
/// list stack vertically; at Medium/Expanded they sit side-by-side. Apps
/// that provide no responsive context fall back to Expanded (desktop-first);
/// pushing a measured width from the app root is what makes this live
/// (loki-text does; see the plan 4c.5 tail for the other apps).
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
/// All interactive elements (template cards, document rows, buttons) meet this.
#[component]
pub fn AtHomeTab(props: AtHomeTabProps) -> Element {
    // F7a: the size class comes from the shared responsive context — the old
    // fixed `viewport_width = 375.0` signal locked this surface to the
    // stacked phone layout everywhere.
    let is_desktop = !use_breakpoint().is_compact();

    // Taffy does not propagate a definite height from a flex:1 child back to
    // its own children's flex:1 items.  Match the Shell outlet's height exactly
    // so that overflow-y:auto on the body div has a definite height to scroll
    // within.  This mirrors the pattern used in shell.rs for the Outlet wrapper.
    let insets = use_safe_area();
    let inset_total = insets.top.round() as u32 + insets.bottom.round() as u32;
    let outer_height = format!("calc(100vh - {}px)", inset_total + TAB_BAR_HEIGHT as u32);

    let mut pick_error = props.pick_error;
    let mut open_hovered = use_signal(|| false);
    let open_bg = if open_hovered() {
        COLOR_ACCENT_PRIMARY_HOVER
    } else {
        COLOR_ACCENT_PRIMARY
    };

    let body_style = if is_desktop {
        format!(
            "display: flex; flex-direction: row; gap: {gap}px; \
             padding: {pad}px; flex: 1; overflow: hidden; min-height: 0;",
            gap = SPACE_4,
            pad = SPACE_4,
        )
    } else {
        format!(
            "display: flex; flex-direction: column; \
             padding: {pad}px; flex: 1; min-height: 0; overflow-y: auto;",
            pad = SPACE_4,
        )
    };

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; \
                 height: {h}; overflow: hidden; \
                 background: {bg}; font-family: {font}; color: {fg};",
                h    = outer_height,
                bg   = COLOR_SURFACE_BASE,
                font = FONT_FAMILY_UI,
                fg   = COLOR_TEXT_PRIMARY,
            ),

            // ── Body: gallery + recent list ───────────────────────────────────
            div {
                style: body_style,

                // Template gallery section
                div {
                    style: if is_desktop {
                        "flex: 1; min-width: 0;".to_string()
                    } else {
                        format!("margin-bottom: {mb}px;", mb = SPACE_6)
                    },
                    h2 {
                        style: format!(
                            "font-size: {size}px; color: {fg}; \
                             margin: 0 0 {mb}px 0; font-weight: {weight};",
                            size   = FONT_SIZE_BODY,
                            fg     = COLOR_TEXT_ON_CHROME_SECONDARY,
                            mb     = SPACE_2,
                            weight = FONT_WEIGHT_SEMIBOLD,
                        ),
                        "{props.templates_label}"
                    }
                    AtTemplateGallery {
                        templates: props.templates.clone(),
                        browse_label: props.browse_label.clone(),
                        on_select: move |idx| { props.on_template_select.call(idx); },
                        on_browse:  move |_|   { props.on_browse_templates.call(()); },
                    }
                }

                // Recent documents section
                div {
                    style: if is_desktop {
                        "flex: 1; min-width: 0; overflow-y: auto;".to_string()
                    } else {
                        String::new()
                    },
                    h2 {
                        style: format!(
                            "font-size: {size}px; color: {fg}; \
                             margin: 0 0 {mb}px 0; font-weight: {weight};",
                            size   = FONT_SIZE_BODY,
                            fg     = COLOR_TEXT_ON_CHROME_SECONDARY,
                            mb     = SPACE_2,
                            weight = FONT_WEIGHT_SEMIBOLD,
                        ),
                        "{props.recent_label}"
                    }
                    AtRecentFileList {
                        documents:       props.recent_documents.clone(),
                        recent_label:    props.recent_label.clone(),
                        empty_label:     props.empty_recent_label.clone(),
                        open_file_label: props.open_file_label.clone(),
                        menu_aria_label: props.recent_menu_aria_label.clone(),
                        remove_label:    props.recent_remove_label.clone(),
                        delete_label:    props.recent_delete_label.clone(),
                        open_copy_label: props.recent_open_copy_label.clone(),
                        on_select:    move |idx| { props.on_recent_open.call(idx); },
                        on_open_file: move |_|   { props.on_open_file.call(()); },
                        on_remove:    move |idx| { props.on_recent_remove.call(idx); },
                        on_delete:    move |idx| { props.on_recent_delete.call(idx); },
                        on_open_copy: move |idx| { props.on_recent_open_copy.call(idx); },
                    }
                }
            }

            // ── Inline error banner ────────────────────────────────────────────
            if let Some(err) = pick_error() {
                div {
                    style: format!(
                        "background: {bg}; border: 1px solid {border}; \
                         margin: {m}px; padding: {p}px; border-radius: {r}px; \
                         color: {fg}; font-size: {size}px;",
                        bg     = COLOR_STATUS_ERROR_BG,
                        border = COLOR_STATUS_ERROR_BORDER,
                        m      = SPACE_4,
                        p      = SPACE_3,
                        r      = RADIUS_SM,
                        fg     = COLOR_STATUS_ERROR_TEXT,
                        size   = FONT_SIZE_LABEL,
                    ),
                    { fl!("error-file-picker", err = err.to_string()) }
                }
            }

            // ── Primary Open File button (bottom) ─────────────────────────────
            // Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).
            div {
                style: format!("padding: {p}px; flex-shrink: 0;", p = SPACE_4),
                button {
                    style: format!(
                        "width: 100%; display: block; margin: 0 auto; \
                         background: {bg}; color: {fg}; \
                         border: none; border-radius: {r}px; \
                         min-height: {touch}px; font-size: {size}px; \
                         font-weight: {weight}; cursor: pointer;",
                        bg     = open_bg,
                        fg     = COLOR_TEXT_ON_CHROME,
                        r      = RADIUS_SM,
                        touch  = TOUCH_MIN,
                        size   = FONT_SIZE_BODY,
                        weight = FONT_WEIGHT_SEMIBOLD,
                    ),
                    onmouseenter: move |_| { open_hovered.set(true); },
                    onmouseleave: move |_| { open_hovered.set(false); },
                    onclick: move |_| {
                        pick_error.set(None);
                        props.on_open_file.call(());
                    },
                    "{props.open_file_label}"
                }
            }
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

/// Props for [`AtHomeTab`].
#[derive(Props, Clone, PartialEq)]
pub struct AtHomeTabProps {
    /// Template entries for the gallery.
    pub templates: Vec<BuiltinTemplate>,

    /// Recently opened documents.
    pub recent_documents: Vec<RecentDocument>,

    // ── Section heading labels ────────────────────────────────────────────────
    /// Heading above the template gallery.
    pub templates_label: String,
    /// Heading above the recent documents list.
    pub recent_label: String,
    /// Label for the "Browse…" card at the end of the template gallery.
    pub browse_label: String,
    /// Label for the primary "Open File" button.
    pub open_file_label: String,
    /// Message shown when `recent_documents` is empty.
    pub empty_recent_label: String,

    // ── Recent document row context-menu labels ───────────────────────────────
    /// Accessible label for the ⋮ button on each recent document row.
    pub recent_menu_aria_label: String,
    /// "Remove from recents" menu item label.
    pub recent_remove_label: String,
    /// "Delete file" menu item label.
    pub recent_delete_label: String,
    /// "Open as copy" menu item label.
    pub recent_open_copy_label: String,

    /// Shared signal holding the current file-picker error message, if any.
    /// Cleared automatically when the user taps "Open File" again.
    pub pick_error: Signal<Option<String>>,

    // ── Callbacks ─────────────────────────────────────────────────────────────
    /// Called when a template card is selected. Argument is the index into `templates`.
    pub on_template_select: EventHandler<usize>,
    /// Called when the "Browse…" card is clicked.
    pub on_browse_templates: EventHandler<()>,
    /// Called when a recent document row is selected. Argument is the index into `recent_documents`.
    pub on_recent_open: EventHandler<usize>,
    /// Called when the "Open File" button is pressed.
    pub on_open_file: EventHandler<()>,
    /// Called when "Remove from recents" is chosen. Argument is the entry index.
    pub on_recent_remove: EventHandler<usize>,
    /// Called when "Delete file" is chosen. Argument is the entry index.
    pub on_recent_delete: EventHandler<usize>,
    /// Called when "Open as copy" is chosen. Argument is the entry index.
    pub on_recent_open_copy: EventHandler<usize>,
}
