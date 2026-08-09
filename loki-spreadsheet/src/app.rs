// SPDX-License-Identifier: Apache-2.0

//! Root application component for loki-spreadsheet.

use appthere_ui::{
    AtPopoverHost, AtThemeContext, AtViewportWidthSensor, ui_font_css, use_provide_popover,
    use_provide_responsive, use_safe_area,
};
use dioxus::prelude::*;

/// The product name shown in the OS window title bar.
pub const WINDOW_TITLE: &str = "Loki Calc";

/// Relative window-geometry persistence path under the platform data dir.
pub const GEOMETRY_FILE: &str = "AppThere/Loki/window_spreadsheet.json";

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

    // Shared responsive context (Spec 03 M1 / audit F7a): seeded unmeasured;
    // the AtViewportWidthSensor below funnels the measured root width in, so
    // AtHomeTab's breakpoint tracks the real window instead of a fallback.
    use_provide_responsive();

    // Window-level dismiss-backdrop context (kept identical to loki-text for
    // suite consistency; used by ribbon overflow menus and anchored popups).
    // Anchored overlays. Provided here as well as in `loki-text` because the
    // shared Home tab's Recent Documents menu is a popover consumer (T4.2), and
    // `use_popover_anchor` degrades to `None` where no root provided the
    // context — which would present as a ⋮ button that does nothing, in two of
    // the three apps, with nothing in the log to say why.
    let _popover = use_provide_popover();
    // **And the window it places against.** `use_provide_popover` alone gives the
    // ⋮ menu a host; without this the host has no measured window, so
    // `usable_viewport` falls back to the consumer's own placeholder — which for
    // every menu in the suite is an *unbounded* rect. An unbounded viewport never
    // flips and never clamps, so a menu near the window bottom opens downward off
    // the screen and reads as a button that does nothing. `loki-text` has provided
    // this since T4.1; these two mounted the host without it (r93).
    let mut window_size = appthere_ui::use_provide_window_size();

    // Spell-check service (bundled English; dictionary cache shared across the
    // suite). Provided into context so this app's editor can query spelling and
    // offer corrections. Visible in-cell squiggles are a follow-up in the
    // spreadsheet's own cell renderer.
    if let Ok(service) = loki_app_shell::spell::SpellService::bootstrap() {
        provide_context(service);
    }

    // Open-document tab list. Index 0 of the Vec = document tab 1
    let tabs: Signal<Vec<OpenTab>> = use_signal(Vec::new);
    let active_tab: Signal<usize> = use_signal(|| 0usize); // 0 = Home tab

    // Recent-documents list.
    let recent_docs: Signal<RecentDocuments> =
        use_signal(|| RecentDocuments::load(crate::recent_documents::RECENT_FILE));

    // Stashed sessions for inactive tabs — unsaved edits survive tab switches.
    let doc_sessions: Signal<crate::sessions::DocSessions> =
        use_signal(std::collections::HashMap::new);

    provide_context(tabs);
    provide_context(active_tab);
    provide_context(recent_docs);
    provide_context(doc_sessions);

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

        // **Registering the face is not selecting it.** The blobs above make
        // "Atkinson Hyperlegible Next" *resolvable*; nothing in the tree asked
        // for it except component-by-component, so an element outside every such
        // component — anything the popover host renders — fell through to the CSS
        // initial value and drew in serif. This sheet is the declaration, at the
        // one place inheritance reaches everything (r94).
        document::Style { "{ui_font_css()}" }

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
                "margin: 0; position: relative; \
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
            // Measure the root width into the responsive context (F7a).
            AtViewportWidthSensor {}
            // Persist the window size across sessions (debounced; desktop only
            // in effect — Android windows are fullscreen).
            // Two consumers of one measurement: the geometry file, and the
            // popover host, which places against the window and has no other way
            // to know its height.
            appthere_ui::AtWindowSizeSensor {
                on_size: move |size: (f64, f64)| {
                    window_size.set(size);
                    loki_app_shell::window_geometry::save_debounced(GEOMETRY_FILE, size)
                },
            }

            Router::<Route> {}

            // Window-level dismiss backdrop; renders nothing while unused.

            // **Must follow `AtBackdropHost`** — `z-index` cannot arbitrate
            // between two children of one positioned root, so DOM order does.
            // See `popover::RootLayer`.
            AtPopoverHost {}
        }
    }
}
