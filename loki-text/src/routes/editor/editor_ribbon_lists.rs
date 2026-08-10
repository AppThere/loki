// SPDX-License-Identifier: Apache-2.0

//! The Write tab's **Lists** group (usage audit §10 tier 3): bullet and
//! numbered toggles plus indent/outdent, over the tier-1 mutations. Keyboard
//! equivalents: Tab / Shift-Tab inside a list item (`editor_keydown`).

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AtIcon, AtRibbonIconButton, LUCIDE_INDENT_DECREASE, LUCIDE_INDENT_INCREASE, LUCIDE_LIST,
    LUCIDE_LIST_ORDERED, RibbonGroupSpec, estimate_group_metrics,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_lists::{ListKind, caret_list_state, change_list_level, toggle_list};
use super::editor_ribbon_format::RibbonEditCtx;
use crate::editing::state::DocumentState;

/// Builds the Lists group. Active states are computed from the caret here at
/// render time (the ribbon already re-renders on caret moves for the style
/// name display).
pub(super) fn lists_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    edit: RibbonEditCtx,
    priority: u8,
) -> RibbonGroupSpec {
    let (bullet_active, numbered_active) = {
        let ldoc_guard = edit.loro_doc.read();
        match ldoc_guard.as_ref() {
            Some(ldoc) => caret_list_state(doc_state, ldoc, &edit.cursor_state.read()),
            None => (false, false),
        }
    };
    let in_list = bullet_active || numbered_active;

    let ds_bullet = Arc::clone(doc_state);
    let ds_numbered = Arc::clone(doc_state);
    let ds_outdent = Arc::clone(doc_state);
    let ds_indent = Arc::clone(doc_state);

    let run_toggle = move |ds: &Arc<Mutex<DocumentState>>, kind: ListKind| {
        let ldoc_guard = edit.loro_doc.read();
        if let Some(ldoc) = ldoc_guard.as_ref()
            && toggle_list(ldoc, &edit.cursor_state.read(), kind)
        {
            edit.finish(ds, ldoc);
        }
    };
    let run_level = move |ds: &Arc<Mutex<DocumentState>>, delta: i8| {
        let ldoc_guard = edit.loro_doc.read();
        if let Some(ldoc) = ldoc_guard.as_ref()
            && change_list_level(ldoc, &edit.cursor_state.read(), delta)
        {
            edit.finish(ds, ldoc);
        }
    };

    RibbonGroupSpec {
        metrics: estimate_group_metrics(priority, 4, true),
        label: Some(fl!("ribbon-group-lists")),
        aria_label: fl!("ribbon-group-lists"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-list-bullet-aria"),
                is_active:   bullet_active,
                is_disabled: false,
                on_click:    move |_| run_toggle(&ds_bullet, ListKind::Bullet),
                AtIcon { path_d: LUCIDE_LIST.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-list-numbered-aria"),
                is_active:   numbered_active,
                is_disabled: false,
                on_click:    move |_| run_toggle(&ds_numbered, ListKind::Numbered),
                AtIcon { path_d: LUCIDE_LIST_ORDERED.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-list-outdent-aria"),
                is_active:   false,
                // Level changes only mean anything inside a list (the
                // mutation refuses otherwise); disabled states say so.
                is_disabled: !in_list,
                on_click:    move |_| run_level(&ds_outdent, -1),
                AtIcon { path_d: LUCIDE_INDENT_DECREASE.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-list-indent-aria"),
                is_active:   false,
                is_disabled: !in_list,
                on_click:    move |_| run_level(&ds_indent, 1),
                AtIcon { path_d: LUCIDE_INDENT_INCREASE.to_string() }
            }
        },
    }
}
