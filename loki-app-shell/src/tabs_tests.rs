// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the pure tab-list operations (`super`).

use super::{OpenTab, TabCloseNav, close_by_path, open_or_switch, resolve_tab_close};

fn tab(path: &str) -> OpenTab {
    OpenTab {
        title: path.to_string(),
        path: path.to_string(),
        is_dirty: false,
        is_discarded: false,
    }
}

// ── open_or_switch ────────────────────────────────────────────────────────────

#[test]
fn open_new_tab_appends_and_returns_last_index() {
    let mut tabs = vec![tab("a"), tab("b")];
    let active = open_or_switch(&mut tabs, "c".into(), "C".into());
    assert_eq!(tabs.len(), 3);
    assert_eq!(tabs[2].path, "c");
    assert_eq!(tabs[2].title, "C");
    assert_eq!(active, 3); // 1-based: third tab, +1 for Home already folded in.
}

#[test]
fn open_existing_tab_switches_without_appending() {
    let mut tabs = vec![tab("a"), tab("b"), tab("c")];
    let active = open_or_switch(&mut tabs, "b".into(), "ignored".into());
    assert_eq!(tabs.len(), 3, "must not append a duplicate");
    assert_eq!(active, 2); // 1-based index of the existing "b" tab.
}

// ── close_by_path ─────────────────────────────────────────────────────────────

#[test]
fn close_by_path_removes_and_resets_active_when_active_at_or_past() {
    let mut tabs = vec![tab("a"), tab("b"), tab("c")];
    // active = 2 (tab-bar index, 1-based) → "b" at vec idx 1. Closing "b"
    // (idx 1): active (2) > idx (1) → reset to Home.
    let active = close_by_path(&mut tabs, 2, "b");
    assert_eq!(
        tabs.iter().map(|t| t.path.as_str()).collect::<Vec<_>>(),
        ["a", "c"]
    );
    assert_eq!(active, 0);
}

#[test]
fn close_by_path_keeps_active_when_active_before_removed() {
    let mut tabs = vec![tab("a"), tab("b"), tab("c")];
    // active = 1 (Home-adjacent first tab), closing "c" at vec idx 2:
    // active (1) is not > idx (2) → unchanged.
    let active = close_by_path(&mut tabs, 1, "c");
    assert_eq!(tabs.len(), 2);
    assert_eq!(active, 1);
}

#[test]
fn close_by_path_absent_leaves_tabs_and_active() {
    let mut tabs = vec![tab("a")];
    let active = close_by_path(&mut tabs, 1, "missing");
    assert_eq!(tabs.len(), 1);
    assert_eq!(active, 1);
}

// ── resolve_tab_close ─────────────────────────────────────────────────────────

#[test]
fn resolve_home_tab_is_noop() {
    assert_eq!(resolve_tab_close(0, 0, 3), None);
}

#[test]
fn resolve_stale_index_is_noop() {
    // idx 4 → vec_idx 3, but only 3 tabs → past the end.
    assert_eq!(resolve_tab_close(4, 1, 3), None);
}

#[test]
fn resolve_closing_last_tab_falls_back_to_home() {
    let out = resolve_tab_close(1, 1, 1).unwrap();
    assert_eq!(out.new_active, 0);
    assert_eq!(out.nav, TabCloseNav::Home);
}

#[test]
fn resolve_closing_active_activates_left_neighbour() {
    // 3 tabs, active = 2 (middle), close it → activate the left neighbour (1).
    let out = resolve_tab_close(2, 2, 3).unwrap();
    assert_eq!(out.new_active, 1);
    assert_eq!(out.nav, TabCloseNav::Editor(0)); // post-removal vec idx 0.
}

#[test]
fn resolve_closing_active_first_tab_activates_new_first() {
    // 3 tabs, active = 1 (first), close it → the new first tab (still index 1).
    let out = resolve_tab_close(1, 1, 3).unwrap();
    assert_eq!(out.new_active, 1);
    assert_eq!(out.nav, TabCloseNav::Editor(0));
}

#[test]
fn resolve_closing_left_of_active_shifts_active_left() {
    // 3 tabs, active = 3, close idx 1 (left of active) → active shifts to 2.
    let out = resolve_tab_close(1, 3, 3).unwrap();
    assert_eq!(out.new_active, 2);
    assert_eq!(out.nav, TabCloseNav::Stay);
}

#[test]
fn resolve_closing_right_of_active_leaves_active() {
    // 3 tabs, active = 1, close idx 3 (right of active) → active unchanged.
    let out = resolve_tab_close(3, 1, 3).unwrap();
    assert_eq!(out.new_active, 1);
    assert_eq!(out.nav, TabCloseNav::Stay);
}
