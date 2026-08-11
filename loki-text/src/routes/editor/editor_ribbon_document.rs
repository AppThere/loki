// SPDX-License-Identifier: Apache-2.0

//! The Write tab's **Document** group: New / Open / Save as split buttons
//! (usage audit §4).
//!
//! Each control pairs a primary action with an anchored menu:
//!
//! - **New** opens a blank document; its menu lists Blank plus every bundled
//!   template (from `loki_templates::TEMPLATES` — the single id list the Home
//!   gallery also renders).
//! - **Open** opens the platform file picker; its menu lists the recent
//!   documents.
//! - **Save** requests a save (untitled documents route to Save As); its menu
//!   holds Save As, Save a Copy, and Save as Template.
//!
//! Extracted to its own module because `editor_ribbon.rs` sits near the
//! 300-line ceiling and this group tripled in behaviour.

use appthere_ui::{
    AtRibbonSplitButton, LUCIDE_FILE_PLUS, LUCIDE_FOLDER_OPEN, LUCIDE_SAVE, RibbonGroupSpec,
    SplitMenuItem, estimate_group_metrics,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_document_actions::BLANK_MENU_ID;
use crate::routes::home_templates::template_display_name;

/// Popover ids for the three menus — distinct per mounted split button.
const NEW_MENU_POPOVER_ID: u64 = 0x000D_0C01;
const OPEN_MENU_POPOVER_ID: u64 = 0x000D_0C02;
const SAVE_MENU_POPOVER_ID: u64 = 0x000D_0C03;

/// Save-menu row ids, matched in `on_save_item`.
const SAVE_AS_ID: &str = "save-as";
const SAVE_A_COPY_ID: &str = "save-a-copy";
const SAVE_AS_TEMPLATE_ID: &str = "save-as-template";

/// How many recent documents the Open menu lists — the menu scrolls past six
/// rows, and a pointer travelling further than this is faster in the Home
/// screen's full list.
const RECENT_MENU_CAP: usize = 8;

/// The callbacks the Document group speaks — built by `EditorInner` (hooks
/// need its scope) and threaded through `write_tab_content`.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct DocumentGroupActions {
    /// New document: `BLANK_MENU_ID` or a bundled-template id.
    pub on_new: Callback<String>,
    /// The platform open picker.
    pub on_open: Callback<()>,
    /// Open a recorded recent path.
    pub on_open_recent: Callback<String>,
    /// Save As (also where untitled saves land).
    pub save_as: Callback<()>,
    /// Export a copy without moving the session.
    pub save_a_copy: Callback<()>,
    /// Export as a `.dotx` template.
    pub save_as_template: Callback<()>,
}

/// The New menu's rows: Blank, then every bundled template.
fn new_menu_items() -> Vec<SplitMenuItem> {
    let mut items = vec![SplitMenuItem {
        id: BLANK_MENU_ID.to_string(),
        label: fl!("home-template-blank"),
    }];
    items.extend(loki_templates::TEMPLATES.iter().map(|t| SplitMenuItem {
        id: t.id.to_string(),
        label: template_display_name(t.id),
    }));
    items
}

/// The Open menu's rows: recent documents (path as id), or one inert
/// placeholder row when there are none (an empty menu reads as broken).
fn recent_menu_items(recents: &[(String, String)]) -> Vec<SplitMenuItem> {
    if recents.is_empty() {
        return vec![SplitMenuItem {
            id: String::new(),
            label: fl!("ribbon-open-no-recents"),
        }];
    }
    recents
        .iter()
        .take(RECENT_MENU_CAP)
        .map(|(path, title)| SplitMenuItem {
            id: path.clone(),
            label: title.clone(),
        })
        .collect()
}

/// Builds the Document group.
///
/// `recents` is `(path, title)` pairs, most-recent first, snapshotted by the
/// caller from the app-context recents store.
pub(super) fn document_group(
    actions: DocumentGroupActions,
    recents: Vec<(String, String)>,
    is_dirty: bool,
    mut save_request: Signal<u32>,
    priority: u8,
) -> RibbonGroupSpec {
    let on_save_item = move |id: String| match id.as_str() {
        SAVE_AS_ID => actions.save_as.call(()),
        SAVE_A_COPY_ID => actions.save_a_copy.call(()),
        SAVE_AS_TEMPLATE_ID => actions.save_as_template.call(()),
        _ => {}
    };

    RibbonGroupSpec {
        // Six 44-px buttons (three split buttons, each a main + chevron pair).
        metrics: estimate_group_metrics(priority, 6, true),
        partial: None,
        label: Some(fl!("ribbon-group-document")),
        aria_label: fl!("ribbon-group-document"),
        content: rsx! {
            AtRibbonSplitButton {
                popover_id:      NEW_MENU_POPOVER_ID,
                aria_label:      fl!("ribbon-new-aria"),
                icon_path:       LUCIDE_FILE_PLUS.to_string(),
                menu_aria_label: fl!("ribbon-new-menu-aria"),
                items:           new_menu_items(),
                on_main:         move |()| actions.on_new.call(BLANK_MENU_ID.to_string()),
                on_item:         move |id| actions.on_new.call(id),
            }
            AtRibbonSplitButton {
                popover_id:      OPEN_MENU_POPOVER_ID,
                aria_label:      fl!("ribbon-open-aria"),
                icon_path:       LUCIDE_FOLDER_OPEN.to_string(),
                menu_aria_label: fl!("ribbon-open-menu-aria"),
                items:           recent_menu_items(&recents),
                on_main:         move |()| actions.on_open.call(()),
                on_item:         move |path| actions.on_open_recent.call(path),
            }
            AtRibbonSplitButton {
                popover_id:      SAVE_MENU_POPOVER_ID,
                aria_label:      fl!("ribbon-save-aria"),
                icon_path:       LUCIDE_SAVE.to_string(),
                // Disabled when clean (plan 4b.3); untitled reads as dirty.
                // The menu stays available — a clean document can still Save
                // As / Save a Copy / Save as Template.
                is_disabled:     !is_dirty,
                menu_aria_label: fl!("ribbon-save-menu-aria"),
                items:           vec![
                    SplitMenuItem { id: SAVE_AS_ID.to_string(), label: fl!("ribbon-save-as-label") },
                    SplitMenuItem { id: SAVE_A_COPY_ID.to_string(), label: fl!("ribbon-save-a-copy-label") },
                    SplitMenuItem {
                        id: SAVE_AS_TEMPLATE_ID.to_string(),
                        label: fl!("ribbon-save-as-template-label"),
                    },
                ],
                on_main: move |()| {
                    // Route through the shared save handler (the Ctrl+S effect
                    // in `EditorInner`), which owns the untitled→Save-As
                    // routing, the clean baseline, status message, and history
                    // compaction.
                    let next = save_request.peek().wrapping_add(1);
                    save_request.set(next);
                },
                on_item: on_save_item,
            }
        },
    }
}

#[cfg(test)]
#[path = "editor_ribbon_document_tests.rs"]
mod tests;
