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

use appthere_ui::{AtThemeContext, use_safe_area};
use dioxus::prelude::*;

use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::tabs::OpenTab;

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

    // Open-document tab list. Index 0 of the Vec = document tab 1 (tab bar
    // index 1, because index 0 is the always-present Home tab).
    let tabs: Signal<Vec<OpenTab>> = use_signal(Vec::new);
    let active_tab: Signal<usize> = use_signal(|| 0usize); // 0 = Home tab

    // Recent-documents list — loaded once from disk at startup.
    let recent_docs: Signal<RecentDocuments> = use_signal(RecentDocuments::load);

    provide_context(tabs);
    provide_context(active_tab);
    provide_context(recent_docs);

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

        // Register Atkinson Hyperlegible Next via CSS @font-face.
        //
        // Blitz processes @font-face rules and fetches the font URL through its
        // net provider.  The `dioxus://` URL is resolved by
        // `dioxus_asset_resolver::native::serve_asset`, which looks for the
        // file relative to the running executable's directory:
        //   debug builds: target/debug/assets/fonts/AtkinsonHyperlegibleNext-VF.ttf
        //   (loki-text/build.rs creates a symlink from target/debug/assets →
        //    loki-text/assets/ so the file is reachable without copying.)
        //
        // The variable font covers all weights (100–900) in a single file,
        // so one @font-face rule covers all weight variants.
        //
        // // TODO(font): verify Atkinson Hyperlegible Next is rendering correctly
        // in the running app — check that chrome text is NOT in system-ui.
        // If the dioxus:// URL fails to resolve (e.g. symlink missing), the
        // FONT_FAMILY_UI fallback chain ("Atkinson Hyperlegible, system-ui")
        // applies automatically.
        document::Style {
            // COMPAT(dioxus-native): CSS @font-face with dioxus:// URLs is
            // confirmed supported — Blitz's net.rs fetch_font_face triggers
            // parley FontContext.collection.register_fonts() on success.
            // The dioxus:// scheme is handled by DioxusNativeNetProvider which
            // delegates to dioxus_asset_resolver::native::serve_asset.
            "@font-face {{
                font-family: 'Atkinson Hyperlegible Next';
                src: url('dioxus:///assets/fonts/AtkinsonHyperlegibleNext-VF.ttf')
                     format('truetype');
                font-weight: 100 900;
                font-style: normal;
            }}"
        }

        div {
            // Shell owns height: 100vh and the flex column layout.
            // Padding offsets the system status bar (top) and navigation bar
            // (bottom) so content is never obscured on Android edge-to-edge.
            // On desktop both insets are 0, so this is a no-op there.
            // COMPAT(dioxus-native): box-sizing: border-box confirmed working.
            style: "margin: 0; \
                    padding: {insets.top}px {insets.right}px {insets.bottom}px {insets.left}px; \
                    width: 100vw; height: 100vh; \
                    overflow: hidden; box-sizing: border-box;",
            Router::<Route> {}
        }
    }
}
