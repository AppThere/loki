// SPDX-License-Identifier: Apache-2.0

//! `AtRecentFileList` — recent documents list with per-row context menu.
//!
//! Rows and the hoverable Open-File button are child `#[component]`s
//! ([`super::recent_row::RecentRow`], [`OpenFileButton`]) so each owns its
//! hook scope — the hover signals used to be `use_signal` calls inside the
//! list's `for` loop and `if` arms, making this component's hook count depend
//! on its props (audit F6a / ADR-0013).

use dioxus::prelude::*;

use super::recent_row::RecentRow;
use crate::components::home_tab::RecentDocument;
use crate::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_ACCENT_PRIMARY_HOVER, COLOR_SURFACE_PAGE, COLOR_TEXT_ON_CHROME,
    COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_SM, SPACE_2, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_WEIGHT_SEMIBOLD};

/// Maximum number of recent entries displayed in the list.
const RECENT_VISIBLE_LIMIT: usize = 10;

// ── AtRecentFileList ──────────────────────────────────────────────────────────

/// Vertically scrollable list of recently opened documents.
///
/// Each row has a primary click area (opens the document) and a ⋮ button
/// that expands an inline context menu with document management actions.
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
/// Both the row click target and the ⋮ button meet this requirement.
#[component]
pub(crate) fn AtRecentFileList(props: AtRecentFileListProps) -> Element {
    let mut menu_open: Signal<Option<usize>> = use_signal(|| None);
    let close_then = move |handler: EventHandler<usize>| {
        EventHandler::new(move |idx: usize| {
            menu_open.set(None);
            handler.call(idx);
        })
    };

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px; \
                 font-family: {font};",
                gap  = SPACE_2,
                font = FONT_FAMILY_UI,
            ),

            if props.documents.is_empty() {
                // ── Empty state ───────────────────────────────────────────────
                div {
                    style: format!(
                        "display: flex; flex-direction: column; \
                         align-items: center; gap: {gap}px; padding: {p}px;",
                        gap = SPACE_4,
                        p   = SPACE_4,
                    ),
                    span {
                        style: format!(
                            "font-size: {size}px; color: {fg};",
                            size = FONT_SIZE_BODY,
                            fg   = COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        "{props.empty_label}"
                    }
                    OpenFileButton {
                        label: props.open_file_label.clone(),
                        full_width: false,
                        on_click: props.on_open_file,
                    }
                }
            }

            for (idx, doc) in props.documents.iter().take(RECENT_VISIBLE_LIMIT).enumerate() {
                RecentRow {
                    key: "{doc.path}",
                    idx,
                    title: doc.title.clone(),
                    modified: doc.modified_at.clone(),
                    is_menu_open: menu_open() == Some(idx),
                    menu_aria_label: props.menu_aria_label.clone(),
                    remove_label: props.remove_label.clone(),
                    delete_label: props.delete_label.clone(),
                    open_copy_label: props.open_copy_label.clone(),
                    on_select: props.on_select,
                    on_toggle_menu: move |i: usize| {
                        menu_open.set(if *menu_open.peek() == Some(i) { None } else { Some(i) });
                    },
                    on_remove: close_then(props.on_remove),
                    on_delete: close_then(props.on_delete),
                    on_open_copy: close_then(props.on_open_copy),
                }
            }

            // Open File button shown below the list when documents exist
            if !props.documents.is_empty() {
                OpenFileButton {
                    label: props.open_file_label.clone(),
                    full_width: true,
                    on_click: props.on_open_file,
                }
            }
        }
    }
}

// ── OpenFileButton ────────────────────────────────────────────────────────────

/// The accent "Open file…" button (hover state owned here, not by the list).
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8)** via
/// `min-height` + padding/width.
#[component]
fn OpenFileButton(label: String, full_width: bool, on_click: EventHandler<()>) -> Element {
    let mut hovered = use_signal(|| false);
    let bg = if hovered() {
        COLOR_ACCENT_PRIMARY_HOVER
    } else {
        COLOR_ACCENT_PRIMARY
    };
    let (fg, sizing) = if full_width {
        (
            COLOR_TEXT_ON_CHROME,
            format!("width: 100%; margin-top: {mt}px;", mt = SPACE_2),
        )
    } else {
        (
            COLOR_SURFACE_PAGE,
            format!("padding: 0 {px}px;", px = SPACE_4),
        )
    };
    rsx! {
        button {
            style: format!(
                "background: {bg}; color: {fg}; \
                 border: none; border-radius: {r}px; \
                 min-height: {touch}px; {sizing} \
                 font-size: {size}px; font-weight: {weight}; \
                 cursor: pointer;",
                bg     = bg,
                fg     = fg,
                r      = RADIUS_SM,
                touch  = TOUCH_MIN,
                sizing = sizing,
                size   = FONT_SIZE_BODY,
                weight = FONT_WEIGHT_SEMIBOLD,
            ),
            onmouseenter: move |_| { hovered.set(true); },
            onmouseleave: move |_| { hovered.set(false); },
            onclick: move |_| { on_click.call(()); },
            "{label}"
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub(crate) struct AtRecentFileListProps {
    pub documents: Vec<RecentDocument>,
    pub recent_label: String,
    pub empty_label: String,
    pub open_file_label: String,
    /// Accessible label for the ⋮ button on each document row.
    pub menu_aria_label: String,
    /// Label for the "Remove from recents" menu action.
    pub remove_label: String,
    /// Label for the "Delete file" menu action.
    pub delete_label: String,
    /// Label for the "Open as copy" menu action.
    pub open_copy_label: String,
    pub on_select: EventHandler<usize>,
    pub on_open_file: EventHandler<()>,
    /// Called with the entry index when "Remove from recents" is chosen.
    pub on_remove: EventHandler<usize>,
    /// Called with the entry index when "Delete file" is chosen.
    pub on_delete: EventHandler<usize>,
    /// Called with the entry index when "Open as copy" is chosen.
    pub on_open_copy: EventHandler<usize>,
}
