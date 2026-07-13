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
use appthere_ui::{AtConfirmDialog, AtDocumentTabData, AtTabBar, use_safe_area};
use dioxus::prelude::*;
use dioxus_router::Navigator;
use loki_i18n::fl;

use super::home_util::push_new_tab;
use crate::new_document::new_blank_tab;
use crate::routes::Route;
use crate::sessions::DocSessions;
use crate::tabs::OpenTab;

/// Closes the tab at 1-based tab-bar index `idx`: drops its stashed editing
/// session (so a later reopen loads fresh from disk instead of resurrecting
/// discarded unsaved edits) and fixes up the active tab / route.
/// The active-tab / navigation arithmetic is the shared, unit-tested
/// [`loki_app_shell::tabs::resolve_tab_close`] decision (deduplicated across the
/// three app shells — plan 7.2); this wrapper applies its outcome to the app's
/// signals, session map, and router.
fn close_tab(
    idx: usize,
    mut tabs: Signal<Vec<OpenTab>>,
    mut active_tab: Signal<usize>,
    mut doc_sessions: Signal<DocSessions>,
    navigator: Navigator,
) {
    use loki_app_shell::tabs::TabCloseNav;

    // Guard (idx captured at event time) lives in `resolve_tab_close`: a stale
    // or Home index yields `None` → no-op.
    let Some(outcome) =
        loki_app_shell::tabs::resolve_tab_close(idx, *active_tab.read(), tabs.read().len())
    else {
        return;
    };

    let vec_idx = idx - 1;
    let closed_path = tabs.read().get(vec_idx).map(|t| t.path.clone());
    if let Some(p) = closed_path {
        doc_sessions.write().remove(&p);
    }
    tabs.write().remove(vec_idx);
    *active_tab.write() = outcome.new_active;

    match outcome.nav {
        // No documents remain — go Home.
        TabCloseNav::Home => {
            navigator.push(Route::Home {});
        }
        // Closed the active tab — navigate to the newly-activated neighbour.
        TabCloseNav::Editor(i) => {
            if let Some(tab) = tabs.read().get(i) {
                navigator.push(Route::Editor {
                    path: tab.path.clone(),
                });
            }
        }
        // Closed a tab beside the active one — index adjusted, displayed
        // document unchanged, no navigation.
        TabCloseNav::Stay => {}
    }
}

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
    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    let mut active_tab = use_context::<Signal<usize>>();
    let doc_sessions = use_context::<Signal<DocSessions>>();
    let navigator = use_navigator();
    // A dirty tab awaiting close confirmation: `(tab-bar index, title)`.
    // While `Some`, the confirmation dialog overlays the shell (plan 4b.6).
    let mut pending_close: Signal<Option<(usize, String)>> = use_signal(|| None);

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
            // position: relative anchors the AtConfirmDialog overlay (its
            // absolute backdrop resolves against this shell-sized box).
            style: format!(
                "height: {shell_height}; \
                 position: relative; \
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
                theme_toggle_aria_label: fl!("shell-theme-toggle-aria"),
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
                    // together with the stashed session (plan 4b.6 / F3c).
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
                    // Open a blank document directly (the Home template
                    // gallery remains reachable via the Home tab).
                    let path = push_new_tab(tabs, active_tab, new_blank_tab());
                    navigator.push(Route::Editor { path });
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
