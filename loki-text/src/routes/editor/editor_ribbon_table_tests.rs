// SPDX-License-Identifier: Apache-2.0

//! Tests for the pure ribbon-tab appearance logic (plan 4a.2): the Table
//! contextual tab appears only while the caret is in a table.

use super::{CONTEXTUAL_TAB_INDEX, ribbon_tabs};
use crate::editing::selected_object::SelectedObject;

#[test]
fn no_selection_shows_only_the_core_tabs() {
    let tabs = ribbon_tabs(SelectedObject::None);
    assert_eq!(tabs.len(), 5, "Write, Insert, Layout, References, Publish");
    assert!(
        tabs.iter().all(|t| !t.is_contextual),
        "core tabs are never contextual",
    );
}

#[test]
fn table_selection_appends_a_contextual_tab() {
    let tabs = ribbon_tabs(SelectedObject::Table);
    assert_eq!(tabs.len(), 6, "the Table tab is appended");
    // The five core tabs stay non-contextual...
    assert!(tabs[..5].iter().all(|t| !t.is_contextual));
    // ...and the appended Table tab is contextual (renders amber).
    assert!(tabs[5].is_contextual, "the Table tab is contextual");
}

#[test]
fn the_contextual_tab_sits_at_the_reserved_index() {
    // The reset logic keys off `active_tab >= CONTEXTUAL_TAB_INDEX`; that index
    // must be exactly where `ribbon_tabs` puts the contextual tab.
    let tabs = ribbon_tabs(SelectedObject::Table);
    assert_eq!(CONTEXTUAL_TAB_INDEX, 5);
    assert!(tabs[CONTEXTUAL_TAB_INDEX].is_contextual);
}
