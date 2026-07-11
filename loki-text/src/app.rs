// SPDX-License-Identifier: Apache-2.0

//! Root application component.
//!
//! [`App`] is the top-level Dioxus component mounted by [`crate::main`].
//! It injects the [`appthere_ui::AtThemeContext`] and the open-tab signals so
//! all shell components can read the active theme variant and tab list, then
//! wraps the Dioxus router with the [`Route`] enum, wiring up client-side
//! navigation between the Home and Editor screens.
//!
//! ## Root layout reset
//!
//! Blitz's user-agent stylesheet applies `body { margin: 8px; }` by default,
//! matching browser behaviour. The injected [`document::Style`] resets this
//! to zero so the application fills the native window edge-to-edge with no
//! visible gap. Without the reset, the 8px body margin causes the root
//! container's `height: 100vh` to overflow (`100vh + 8px`), making the
//! window vertically scrollable.

use appthere_ui::tokens;
use appthere_ui::{
    AtBackdropHost, AtThemeContext, use_provide_backdrop, use_provide_responsive, use_safe_area,
};
use dioxus::prelude::*;

use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::sessions::DocSessions;
use crate::tabs::OpenTab;

/// Query the current orientation-aware safe-area insets, falling back to the
/// orientation-independent resource heights (status/navigation bar) before the
/// window is laid out or on API levels without `getInsets(int)`.
///
/// The query folds in the soft-keyboard (IME) inset, so when the keyboard is
/// visible the returned `bottom` grows to the keyboard height. Blitz drives the
/// re-query as the keyboard animates (the IME-settle re-sync in `blitz-shell`),
/// so the bottom padding tracks the keyboard and the ribbon / bottom-of-document
/// content is never hidden behind it.
#[cfg(target_os = "android")]
fn current_safe_area() -> appthere_ui::SafeAreaInsets {
    let activity = blitz_shell::current_android_app().activity_as_ptr();
    if let Some((top, bottom, left, right)) = loki_file_access::query_window_insets_dp(activity) {
        appthere_ui::SafeAreaInsets {
            top,
            bottom,
            left,
            right,
        }
    } else {
        let (top, bottom) = loki_file_access::query_insets_dp();
        appthere_ui::SafeAreaInsets {
            top,
            bottom,
            ..Default::default()
        }
    }
}

/// Hidden zero-size scroll container that re-queries orientation-aware
/// safe-area insets on every resize.
///
/// The blitz shell re-emits `onscroll` to every scroll container after a resize
/// (`resync_scroll_geometry`); this element catches that tick app-wide and
/// updates the reactive insets, so rotating to landscape no longer keeps the
/// portrait padding (which over-condenses the usable area and pads the wrong
/// edges when the navigation bar / cutout move to a side). On desktop it renders
/// nothing.
///
/// The blitz shell also re-emits this tick while the soft keyboard is animating
/// in or out (it has no surface resize to react to on a `NativeActivity`), so
/// the same sensor refreshes the bottom inset to track the keyboard.
#[component]
fn SafeAreaResizeSensor() -> Element {
    #[cfg(target_os = "android")]
    return rsx! {
        div {
            style: "width: 0px; height: 0px; overflow: auto;",
            onscroll: move |_| {
                appthere_ui::update_safe_area_insets(current_safe_area());
            },
        }
    };
    #[cfg(not(target_os = "android"))]
    rsx! {}
}

/// Root application component.
///
/// Injects [`AtThemeContext`] (defaults to `ThemeVariant::Dark`) and two tab
/// signals before any shell component renders, then mounts the [`Router`].
/// All navigation state lives inside the router; components call
/// [`use_navigator`] to push or replace routes programmatically.
#[component]
pub fn App() -> Element {
    // Inject the theme context before any shell component renders.
    provide_context(AtThemeContext::default());

    // Provide the shared responsive context (Spec 03 M1). Seeded unmeasured
    // (→ Breakpoint::Compact); the editor funnels the one measured scroll-
    // container width into it (no second width source). Descendants read the
    // derived breakpoint via `appthere_ui::use_breakpoint`.
    use_provide_responsive();

    // Window-level dismiss-backdrop context (outside-click-to-close for the
    // ribbon overflow menu and future anchored popups); AtBackdropHost below
    // renders the active backdrop inside this positioned root.
    use_provide_backdrop();

    // Spell-check service — starts on the bundled English dictionary so checking
    // works offline. Provided into context for any component (e.g. a future
    // language picker), and installed into the editor's ambient layout state so
    // the layout engine paints squiggles under misspelled words.
    match loki_app_shell::spell::SpellService::bootstrap() {
        Ok(service) => {
            crate::editing::spell::set_active(service.snapshot().map(|snap| {
                loki_layout::SpellState {
                    checker: snap.checker,
                    generation: snap.generation,
                }
            }));
            provide_context(service);
        }
        Err(err) => tracing::warn!("spell check unavailable: {err}"),
    }

    // Open-document tab list. Index 0 of the Vec = document tab 1 (tab bar
    // index 1, because index 0 is the always-present Home tab).
    let tabs: Signal<Vec<OpenTab>> = use_signal(Vec::new);
    let active_tab: Signal<usize> = use_signal(|| 0usize); // 0 = Home tab

    // Recent-documents list — loaded once from disk at startup.
    let recent_docs: Signal<RecentDocuments> =
        use_signal(|| RecentDocuments::load(crate::recent_documents::RECENT_FILE));

    // Stashed editing sessions for inactive document tabs — unsaved edits
    // survive tab switches by round-tripping through this map.
    let doc_sessions: Signal<DocSessions> = use_signal(DocSessions::new);

    provide_context(tabs);
    provide_context(active_tab);
    provide_context(recent_docs);
    provide_context(doc_sessions);

    // Read OS-reported system-bar insets (non-zero on Android edge-to-edge).
    // Zero on desktop platforms, so this has no visual effect there.
    let insets = use_safe_area();

    rsx! {
        // Reset the body margin injected by Blitz's user-agent stylesheet so
        // the app fills the native window without an 8px gap on every edge.
        document::Style {
            "
            html, body, main {{
                margin: 0;
                padding: 0;
                overflow: hidden;
                height: 100%;
            }}
            "
        }

        // The UI typeface (Atkinson Hyperlegible Next) and the bundled
        // metric-compatible fallback families (Carlito/Caladea/Arimo/Cousine/
        // Tinos) are registered synchronously into the renderer's font collection
        // at launch via `dioxus::native::Config::with_fonts(loki_fonts::ui_font_blobs())`
        // (see `main.rs` / `android_main`). That replaces the previous
        // `@font-face` `data:` URI injection, which relied on the asynchronous
        // resource-fetch path and did not load reliably on Android.

        div {
            // Shell owns height: 100vh and the flex column layout.
            // Padding offsets the system status bar (top) and navigation bar
            // (bottom) so content is never obscured on Android edge-to-edge.
            // When the soft keyboard is visible the bottom inset grows to the
            // keyboard height, pushing the ribbon / bottom content above it.
            // On desktop all insets are 0, so this is a no-op there.
            // background matches COLOR_SURFACE_CHROME so the padded system-bar
            // areas (notification bar at top, gesture strip at bottom) are filled
            // with the tab-bar chrome color instead of the default white.
            // COMPAT(dioxus-native): box-sizing: border-box confirmed working.
            // position: relative hosts the window-level dismiss backdrop
            // (AtBackdropHost) and any future root-anchored overlay.
            style: format!(
                "margin: 0; position: relative; \
                 padding: {top}px {right}px {bottom}px {left}px; \
                 width: 100vw; height: 100vh; \
                 overflow: hidden; box-sizing: border-box; \
                 background: {bg};",
                // Round each inset to the nearest integer so the CSS pixel
                // values match the rounded values used by Shell's calc()
                // expressions.  Without rounding, sub-pixel dp values (e.g.
                // 33.52 on a Pixel 6 at density 2.625) produce a fractional
                // padding that is resolved differently from Shell's integer
                // subtraction, leaving a hairline gap on high-density displays.
                top    = insets.top.round() as i32,
                right  = insets.right.round() as i32,
                bottom = insets.bottom.round() as i32,
                left   = insets.left.round() as i32,
                bg     = tokens::COLOR_SURFACE_CHROME,
            ),

            // Re-query safe-area insets on resize (Android orientation change).
            SafeAreaResizeSensor {}

            Router::<Route> {}

            // Window-level dismiss backdrop (e.g. the ribbon overflow menu's
            // outside-click-to-close). Renders nothing while no popup is open.
            AtBackdropHost {}
        }
    }
}
