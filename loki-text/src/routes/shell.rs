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
use appthere_ui::{AtDocumentTabData, AtTabBar, use_safe_area};
use dioxus::prelude::*;
use loki_i18n::fl;

use crate::routes::Route;
use crate::tabs::OpenTab;

/// Persistent application shell.
///
/// Reads `Signal<Vec<OpenTab>>` and `Signal<usize>` from Dioxus context
/// (injected at the [`crate::app::App`] root) to drive [`AtTabBar`].
///
/// Height is `calc(100vh − safe_top − safe_bottom)` so it fits exactly within
/// the inset-padded content area of the App root div on edge-to-edge platforms.
/// Child routes must use `flex: 1` on their outermost div so they fill the
/// space below the tab bar.
#[component]
pub fn Shell() -> Element {
    let mut tabs = use_context::<Signal<Vec<OpenTab>>>();
    let mut active_tab = use_context::<Signal<usize>>();
    let navigator = use_navigator();

    // Safe-area insets are set by android_main from the OS-reported system-bar
    // heights before Dioxus launches. On desktop they are always (0, 0, 0, 0).
    // Shell subtracts them so its height never exceeds the padded content area
    // inside the App root div, preventing the ribbon from being clipped off-screen
    // by the bottom inset on devices with gesture navigation.
    let insets = use_safe_area();

    // Pre-sum insets so the calc() expression uses a single subtraction term.
    // COMPAT(dioxus-native): `calc(100vh - Npx)` (single term) is confirmed
    // working in Blitz/Taffy. Multi-term expressions (`100vh - Xpx - Ypx`) are
    // unconfirmed and may not resolve to a definite length in Taffy, which would
    // produce a zero/NaN height and panic during scene composition.
    // Round each inset individually before summing so the total matches the sum
    // of the per-side pixel values written by App (e.g. top=34 + bottom=34 = 68,
    // not round(33.52 + 33.52) = 67).
    let inset_top_px = insets.top.round() as u32;
    let inset_bottom_px = insets.bottom.round() as u32;
    let inset_total_px = inset_top_px + inset_bottom_px;
    // calc(100vh - 0px) is identical to 100vh in CSS; no special case needed.
    let shell_height = format!("calc(100vh - {inset_total_px}px)");
    let outlet_height = {
        let total = inset_total_px + tokens::TAB_BAR_HEIGHT as u32;
        format!("calc(100vh - {total}px)")
    };

    rsx! {
        div {
            // COMPAT(dioxus-native): explicit calc(100vh - Npx) gives Taffy a
            // definite length for child flex: 1 to resolve against. The safe-area
            // insets are pre-summed into a single value so the expression stays in
            // the `calc(100vh - Npx)` form that is confirmed working in Blitz.
            style: format!(
                "height: {shell_height}; \
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
                    // Guard: idx is captured at render time; a rapid second close event
                    // before the deferred DOM re-render produces a stale vec_idx that is
                    // already out of bounds after the first removal.
                    if vec_idx >= tabs.read().len() {
                        return;
                    }
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
            // The insets are subtracted alongside the tab bar so the calculation
            // stays consistent with the Shell container height above.
            div {
                style: format!(
                    "height: {outlet_height}; \
                     overflow: hidden; display: flex; flex-direction: column;",
                ),
                Outlet::<Route> {}
            }
        }
    }
}
