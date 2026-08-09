// SPDX-License-Identifier: Apache-2.0

//! `AtRecentFileList` — recent documents list with per-row context menu.
//!
//! Rows are child `#[component]`s ([`super::recent_row::RecentRow`]) so each
//! owns its hook scope — the hover signals used to be `use_signal` calls inside
//! the list's `for` loop, making this component's hook count depend on its
//! props (audit F6a / ADR-0013).
//!
//! The list carries **no Open-File button of its own**. It had two — one below
//! the rows and one in the empty state — and with the heading's Open action
//! (T4.3) directly above, a populated Home screen showed the same control three
//! times. The heading's is the one that survives, because it is the one that
//! does not scroll away.

use std::rc::Rc;

use dioxus::prelude::*;

use super::recent_menu::{key_for_path, RecentMenuActions, RecentMenuPopover, RecentMenuTarget};
use super::recent_row::RecentRow;
use crate::components::home_tab::RecentDocument;
use crate::components::popover::Rect;
use crate::tokens::colors::COLOR_TEXT_ON_CHROME_SECONDARY;
use crate::tokens::spacing::{SPACE_2, SPACE_4};
use crate::tokens::typography::FONT_SIZE_BODY;

/// Maximum number of recent entries displayed in the list.
const RECENT_VISIBLE_LIMIT: usize = 10;

// ── AtRecentFileList ──────────────────────────────────────────────────────────

/// Vertically scrollable list of recently opened documents.
///
/// Each row has a primary click area (opens the document) and a ⋮ button that
/// opens an **anchored popover** with document-management actions (Spec 08
/// T4.2). The menu was inline and expanding until then; see `recent_menu` for
/// why this list's `overflow-y: auto` made that unfixable in place.
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
/// Both the row click target and the ⋮ button meet this requirement.
#[component]
pub(crate) fn AtRecentFileList(props: AtRecentFileListProps) -> Element {
    // **Keyed on the document, not on its position (T4.2).** This was
    // `Signal<Option<usize>>`, and an index is not an identity: the rows are
    // keyed by path, so a `documents` prop that changes while a menu is open
    // leaves the menu attached to whatever document now sits at that index —
    // and `Delete file` is one of its actions. `RecentMenuTarget` carries a
    // path-derived key, and `RecentMenuPopover` checks it before anything can
    // fire.
    let mut menu_open: Signal<Option<RecentMenuTarget>> = use_signal(|| None);
    // The ⋮ button of whichever row last opened a menu, for focus restoration
    // (T4.5). One slot rather than one per row, because the singleton rule
    // means only one menu is open at a time — see `RecentRowProps::anchor_el`.
    let anchor_el: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    // The paths currently displayed, in order — the identity check's input.
    // Derived from the same `take(..)` the rows use, so "position 3" means the
    // same thing on both sides (L08-029).
    let visible_paths: Vec<String> = props
        .documents
        .iter()
        .take(RECENT_VISIBLE_LIMIT)
        .map(|doc| doc.path.clone())
        .collect();

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px;",
                gap  = SPACE_2,
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
                }
            }

            for (idx, doc) in props.documents.iter().take(RECENT_VISIBLE_LIMIT).enumerate() {
                RecentRow {
                    key: "{doc.path}",
                    idx,
                    title: doc.title.clone(),
                    modified: doc.modified_at.clone(),
                    is_menu_open: menu_open
                        .read()
                        .as_ref()
                        .is_some_and(|t| t.index == idx),
                    menu_aria_label: props.menu_aria_label.clone(),
                    on_select: props.on_select,
                    anchor_el,
                    on_toggle_menu: {
                        let path = doc.path.clone();
                        move |(i, rect): (usize, Option<Rect>)| {
                            let already_open = menu_open
                                .peek()
                                .as_ref()
                                .is_some_and(|t| t.index == i);
                            // A missing rect closes rather than opens: there is
                            // nowhere to anchor, and an overlay placed at a
                            // guessed position is worse than none.
                            match rect.filter(|_| !already_open) {
                                Some(anchor) => menu_open.set(Some(RecentMenuTarget {
                                    key: key_for_path(&path),
                                    index: i,
                                    anchor,
                                })),
                                None => menu_open.set(None),
                            }
                        }
                    },
                }
            }

            // The menu, mounted at the boundary so it owns a hook scope
            // (ADR-0013). It renders nothing here — `AtPopoverHost` renders it
            // at the app root, which is what takes it out of this list's
            // `overflow-y: auto` (I-08).
            if let Some(target) = menu_open.read().clone() {
                RecentMenuPopover {
                    target,
                    anchor_el,
                    paths: visible_paths.clone(),
                    actions: RecentMenuActions {
                        remove_label: props.remove_label.clone(),
                        delete_label: props.delete_label.clone(),
                        open_copy_label: props.open_copy_label.clone(),
                        on_remove: props.on_remove,
                        on_delete: props.on_delete,
                        on_open_copy: props.on_open_copy,
                    },
                    on_dismiss: move |()| menu_open.set(None),
                }
            }
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub(crate) struct AtRecentFileListProps {
    pub documents: Vec<RecentDocument>,
    pub empty_label: String,
    /// Accessible label for the ⋮ button on each document row.
    pub menu_aria_label: String,
    /// Label for the "Remove from recents" menu action.
    pub remove_label: String,
    /// Label for the "Delete file" menu action.
    pub delete_label: String,
    /// Label for the "Open as copy" menu action.
    pub open_copy_label: String,
    pub on_select: EventHandler<usize>,
    /// Called with the entry index when "Remove from recents" is chosen.
    pub on_remove: EventHandler<usize>,
    /// Called with the entry index when "Delete file" is chosen.
    pub on_delete: EventHandler<usize>,
    /// Called with the entry index when "Open as copy" is chosen.
    pub on_open_copy: EventHandler<usize>,
}
