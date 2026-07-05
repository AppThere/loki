// SPDX-License-Identifier: Apache-2.0

//! Persistent application shell wrapping all routes for loki-presentation.

use appthere_ui::tokens;
use appthere_ui::{AtConfirmDialog, AtDocumentTabData, AtTabBar};
use dioxus::prelude::*;
use dioxus_router::Navigator;
use loki_i18n::fl;

use crate::routes::Route;
use crate::sessions::DocSessions;
use crate::tabs::OpenTab;

/// Closes the tab at 1-based tab-bar index `idx`: drops its stashed editing
/// session (so a later reopen loads fresh from disk instead of resurrecting
/// discarded unsaved edits) and fixes up the active tab / route.
fn close_tab(
    idx: usize,
    mut tabs: Signal<Vec<OpenTab>>,
    mut active_tab: Signal<usize>,
    mut doc_sessions: Signal<DocSessions>,
    navigator: Navigator,
) {
    let vec_idx = idx - 1;
    // Guard: idx is captured at event time; a rapid second close (or a close
    // confirmed after the list changed) must not index out of bounds.
    if vec_idx >= tabs.read().len() {
        return;
    }
    let current_active = *active_tab.read();

    let closed_path = tabs.read().get(vec_idx).map(|t| t.path.clone());
    if let Some(p) = closed_path {
        doc_sessions.write().remove(&p);
    }

    tabs.write().remove(vec_idx);
    let new_len = tabs.read().len();

    if new_len == 0 {
        *active_tab.write() = 0;
        navigator.push(Route::Home {});
    } else if idx == current_active {
        let new_active = if vec_idx > 0 { idx - 1 } else { 1 };
        *active_tab.write() = new_active;
        if let Some(tab) = tabs.read().get(new_active - 1) {
            navigator.push(Route::Editor {
                path: tab.path.clone(),
            });
        }
    } else if idx < current_active {
        *active_tab.write() = current_active - 1;
    }
}

/// Persistent application shell.
#[component]
pub fn Shell() -> Element {
    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    let mut active_tab = use_context::<Signal<usize>>();
    let doc_sessions = use_context::<Signal<DocSessions>>();
    let navigator = use_navigator();
    // A dirty tab awaiting close confirmation: `(tab-bar index, title)`.
    // While `Some`, the confirmation dialog overlays the shell (plan 4b.6).
    let mut pending_close: Signal<Option<(usize, String)>> = use_signal(|| None);

    rsx! {
        div {
            // position: relative anchors the AtConfirmDialog overlay (its
            // absolute backdrop resolves against this shell-sized box).
            style: format!(
                "height: 100vh; position: relative; \
                 display: flex; flex-direction: column; \
                 overflow: hidden; background: {bg};",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // ── Tab bar (always visible across all routes) ────────────────────
            AtTabBar {
                tabs: tabs.read().iter().map(|t| AtDocumentTabData {
                    title:        t.title.clone(),
                    is_dirty:     t.is_dirty,
                    is_discarded: t.is_discarded,
                }).collect(),
                active_index:       *active_tab.read(),
                home_tab_label:     fl!("shell-home-tab"),
                aria_label:         fl!("shell-tab-bar-aria"),
                new_tab_aria_label: fl!("shell-new-document-aria"),
                on_tab_select: move |idx: usize| {
                    *active_tab.write() = idx;
                    if idx == 0 {
                        navigator.push(Route::Home {});
                    } else if let Some(tab) = tabs.read().get(idx - 1) {
                        navigator.push(Route::Editor { path: tab.path.clone() });
                    }
                },
                on_tab_close: move |idx: usize| {
                    if idx == 0 {
                        return; // Home tab cannot be closed.
                    }
                    let vec_idx = idx - 1;
                    // A dirty tab gets a confirmation dialog instead of an
                    // immediate close — closing discards its unsaved edits
                    // together with the stashed session (plan 4b.6).
                    let dirty_title = tabs
                        .read()
                        .get(vec_idx)
                        .and_then(|t| t.is_dirty.then(|| t.title.clone()));
                    if let Some(title) = dirty_title {
                        pending_close.set(Some((idx, title)));
                        return;
                    }
                    close_tab(idx, tabs, active_tab, doc_sessions, navigator);
                },
                on_new_tab: move |_| {
                    *active_tab.write() = 0;
                    navigator.push(Route::Home {});
                },
            }

            // ── Route outlet (fills remaining vertical space) ─────────────────
            // COMPAT(dioxus-native): explicit calc height required — see loki-text shell.rs.
            div {
                style: format!(
                    "height: calc(100vh - {h}px); overflow: hidden; \
                     display: flex; flex-direction: column;",
                    h = tokens::TAB_BAR_HEIGHT,
                ),
                Outlet::<Route> {}
            }

            // ── Dirty-tab close confirmation (ADR-0013 boundary mount) ────────
            {pending_close.read().clone().map(|(idx, title)| rsx! {
                AtConfirmDialog {
                    title: fl!("shell-close-dirty-title"),
                    message: fl!("shell-close-dirty-message", title = title),
                    confirm_label: fl!("shell-close-dirty-confirm"),
                    cancel_label: fl!("shell-close-dirty-cancel"),
                    on_confirm: move |_| {
                        pending_close.set(None);
                        close_tab(idx, tabs, active_tab, doc_sessions, navigator);
                    },
                    on_cancel: move |_| pending_close.set(None),
                }
            })}
        }
    }
}
