// SPDX-License-Identifier: Apache-2.0

//! Persistent application shell wrapping all routes.
//!
//! [`Shell`] renders the tab bar around the router [`Outlet`] so it survives
//! route transitions without being re-mounted.  The ribbon and status bar are
//! owned by [`crate::routes::editor::editor_inner::EditorInner`] so they only
//! appear when a document is open.
//!
//! Layout:
//! ```text
//! ┌─────────────────────────────────────────┐ ← height: 100vh
//! │  AtTabBar        (flex-shrink: 0)        │
//! ├─────────────────────────────────────────┤
//! │  Outlet (Home or Editor)  (flex: 1)      │
//! └─────────────────────────────────────────┘
//! ```

use appthere_ui::tokens;
use appthere_ui::{AtDocumentTabData, AtTabBar};
use dioxus::prelude::*;
use loki_i18n::fl;

use crate::routes::Route;
use crate::tabs::OpenTab;

/// Persistent application shell.
///
/// Reads `Signal<Vec<OpenTab>>` and `Signal<usize>` from Dioxus context
/// (injected at the [`crate::app::App`] root) to drive [`AtTabBar`].
///
/// **Height: 100vh** — this component owns the full viewport height constraint.
/// Child routes must use `flex: 1` on their outermost div so they fill the
/// space below the tab bar.
#[component]
pub fn Shell() -> Element {
    let mut tabs = use_context::<Signal<Vec<OpenTab>>>();
    let mut active_tab = use_context::<Signal<usize>>();
    let navigator = use_navigator();

    rsx! {
        div {
            // COMPAT(dioxus-native): height: 100vh is confirmed working and
            // gives Taffy a definite length for child flex: 1 to resolve
            // against. This is the single element asserting the full viewport
            // height — all child route components use flex: 1 instead.
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
                        // No documents remain — go Home.
                        *active_tab.write() = 0;
                        navigator.push(Route::Home {});
                    } else if idx == current_active {
                        // Closed the active tab — activate the nearest remaining tab.
                        // Prefer the tab to the left; fall back to the first tab.
                        let new_active = if vec_idx > 0 { idx - 1 } else { 1 };
                        *active_tab.write() = new_active;
                        if let Some(tab) = tabs.read().get(new_active - 1) {
                            navigator.push(Route::Editor { path: tab.path.clone() });
                        }
                    } else if idx < current_active {
                        // Closed a tab to the LEFT of the active tab — the Vec
                        // shifted so the active document's index decrements by 1.
                        // Do NOT navigate: the displayed document is unchanged.
                        *active_tab.write() = current_active - 1;
                    }
                    // Closing a tab to the RIGHT of the active tab: no index
                    // change and no navigation — displayed document is unchanged.
                },
                on_new_tab: move |_| {
                    // Navigate to Home so the user can pick a template or file.
                    // TODO(tabs): Open a blank document directly once blank-doc
                    // creation is implemented.
                    *active_tab.write() = 0;
                    navigator.push(Route::Home {});
                },
            }

            // ── Route outlet (fills remaining vertical space) ─────────────────
            // COMPAT(dioxus-native): Taffy does not propagate a definite height
            // from a flex:1 child back into its own children's flex:1 items.
            // Using height:calc(100vh - Npx) gives an explicit definite length
            // that Taffy resolves correctly — required for overflow-y:auto scroll
            // to engage in both the home and editor route content.
            div {
                style: format!(
                    "height: calc(100vh - {h}px); overflow: hidden; \
                     display: flex; flex-direction: column;",
                    h = tokens::TAB_BAR_HEIGHT,
                ),
                Outlet::<Route> {}
            }
        }
    }
}
