// SPDX-License-Identifier: Apache-2.0

//! Persistent application shell wrapping all routes for loki-spreadsheet.

use appthere_ui::tokens;
use appthere_ui::{AtDocumentTabData, AtTabBar};
use dioxus::prelude::*;
use loki_i18n::fl;

use crate::routes::Route;
use crate::tabs::OpenTab;

/// Persistent application shell.
#[component]
pub fn Shell() -> Element {
    let mut tabs = use_context::<Signal<Vec<OpenTab>>>();
    let mut active_tab = use_context::<Signal<usize>>();
    let navigator = use_navigator();

    rsx! {
        div {
            style: format!(
                "height: 100vh; display: flex; flex-direction: column; \
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
                    let current_active = *active_tab.read();

                    tabs.write().remove(vec_idx);
                    let new_len = tabs.read().len();

                    if new_len == 0 {
                        *active_tab.write() = 0;
                        navigator.push(Route::Home {});
                    } else if idx == current_active {
                        let new_active = if vec_idx > 0 { idx - 1 } else { 1 };
                        *active_tab.write() = new_active;
                        if let Some(tab) = tabs.read().get(new_active - 1) {
                            navigator.push(Route::Editor { path: tab.path.clone() });
                        }
                    } else if idx < current_active {
                        *active_tab.write() = current_active - 1;
                    }
                },
                on_new_tab: move |_| {
                    *active_tab.write() = 0;
                    navigator.push(Route::Home {});
                },
            }

            // ── Route outlet (fills remaining vertical space) ─────────────────
            div {
                style: "flex: 1; overflow: hidden; \
                        display: flex; flex-direction: column;",
                Outlet::<Route> {}
            }
        }
    }
}
