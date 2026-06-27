// SPDX-License-Identifier: Apache-2.0

//! Root application component for loki-spreadsheet.

use appthere_ui::{AtThemeContext, use_safe_area};
use dioxus::prelude::*;

use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::tabs::OpenTab;

/// Query the current orientation-aware safe-area insets, falling back to the
/// orientation-independent resource heights (status/navigation bar) before the
/// window is laid out or on API levels without `getInsets(int)`.
///
/// The query folds in the soft-keyboard (IME) inset, so when the keyboard is
/// visible the returned `bottom` grows to the keyboard height. Blitz drives the
/// re-query as the keyboard animates (the IME-settle re-sync in `blitz-shell`),
/// so the bottom padding tracks the keyboard and the toolbar / bottom-of-sheet
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
/// safe-area insets on every resize and while the soft keyboard animates.
///
/// The blitz shell re-emits `onscroll` to every scroll container after a resize
/// (`resync_scroll_geometry`) and across the IME show/hide animation (it has no
/// surface resize to react to on a `NativeActivity`); this element catches that
/// tick app-wide and updates the reactive insets, so rotating to landscape no
/// longer keeps the portrait padding and the soft keyboard pushes content above
/// itself. On desktop it renders nothing.
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
#[component]
pub fn App() -> Element {
    // Inject the theme context before any shell component renders.
    provide_context(AtThemeContext::default());

    // Open-document tab list. Index 0 of the Vec = document tab 1
    let tabs: Signal<Vec<OpenTab>> = use_signal(Vec::new);
    let active_tab: Signal<usize> = use_signal(|| 0usize); // 0 = Home tab

    // Recent-documents list.
    let recent_docs: Signal<RecentDocuments> =
        use_signal(|| RecentDocuments::load(crate::recent_documents::RECENT_FILE));

    provide_context(tabs);
    provide_context(active_tab);
    provide_context(recent_docs);

    let insets = use_safe_area();

    rsx! {
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

        // The UI typeface and bundled fallback families are registered
        // synchronously into the renderer's font collection at launch via
        // `dioxus::native::Config::with_fonts(loki_fonts::ui_font_blobs())` (see
        // `main.rs` / `android_main`), replacing the previous `@font-face`
        // `data:` URI injection that did not load reliably on Android.

        div {
            // Padding offsets the system status/navigation bars on Android
            // edge-to-edge; the bottom inset grows to the soft-keyboard height
            // so content lifts above it. On desktop all insets are 0 (no-op).
            // background matches COLOR_SURFACE_CHROME so the padded system-bar
            // areas are filled with the chrome color instead of default white,
            // and each inset is rounded to an integer so the CSS pixel values
            // match Shell's integer calc() subtraction (avoids hairline gaps on
            // high-density displays). Kept identical to loki-text for suite
            // consistency.
            style: format!(
                "margin: 0; \
                 padding: {top}px {right}px {bottom}px {left}px; \
                 width: 100vw; height: 100vh; \
                 overflow: hidden; box-sizing: border-box; \
                 background: {bg};",
                top    = insets.top.round() as i32,
                right  = insets.right.round() as i32,
                bottom = insets.bottom.round() as i32,
                left   = insets.left.round() as i32,
                bg     = appthere_ui::tokens::COLOR_SURFACE_CHROME,
            ),

            // Re-query safe-area insets on resize (orientation change) and while
            // the soft keyboard animates in/out (Android).
            SafeAreaResizeSensor {}

            Router::<Route> {}
        }
    }
}
