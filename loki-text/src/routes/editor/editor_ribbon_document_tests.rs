// SPDX-License-Identifier: Apache-2.0

//! The Document group's menu tables — the New menu's single id list and the
//! Open menu's recents behaviour.

use super::{RECENT_MENU_CAP, new_menu_items, recent_menu_items};
use crate::routes::editor::editor_document_actions::BLANK_MENU_ID;

/// The New menu is Blank followed by `loki_templates::TEMPLATES`, in order —
/// the same list the Home gallery renders, so the two surfaces cannot offer
/// different templates.
#[test]
fn new_menu_is_blank_then_the_template_crate_list() {
    let items = new_menu_items();
    assert_eq!(items[0].id, BLANK_MENU_ID);
    let ids: Vec<&str> = items[1..].iter().map(|i| i.id.as_str()).collect();
    let expected: Vec<&str> = loki_templates::TEMPLATES.iter().map(|t| t.id).collect();
    assert_eq!(ids, expected);
}

/// `BLANK_MENU_ID` is the one id the New callback treats specially, so no
/// bundled template may ever claim it — mechanical, because a collision would
/// silently open a blank document instead of the template.
#[test]
fn blank_id_cannot_collide_with_a_template_id() {
    assert!(
        loki_templates::TEMPLATES
            .iter()
            .all(|t| t.id != BLANK_MENU_ID)
    );
}

#[test]
fn recents_menu_caps_and_keeps_order() {
    let recents: Vec<(String, String)> = (0..20)
        .map(|i| (format!("path-{i}"), format!("Title {i}")))
        .collect();
    let items = recent_menu_items(&recents);
    assert_eq!(items.len(), RECENT_MENU_CAP);
    assert_eq!(items[0].id, "path-0", "most-recent first");
    assert_eq!(items[0].label, "Title 0");
}

/// No recents → one placeholder row whose empty id the open-recent callback
/// ignores; an empty menu would read as a broken control.
#[test]
fn empty_recents_yield_an_inert_placeholder() {
    let items = recent_menu_items(&[]);
    assert_eq!(items.len(), 1);
    assert!(items[0].id.is_empty());
    assert!(!items[0].label.is_empty());
}
