// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`AtViewportWidthSensor`] — pushes the root width into the responsive
//! context for apps without another measured width source (audit F7a).

use dioxus::prelude::*;

use super::{use_responsive, Viewport};

/// Invisible, zero-height, full-width scroll container that measures its own
/// width into the shared responsive context: once at mount, and again on every
/// shell `resync_scroll_geometry` tick (the blitz shell re-emits `onscroll` to
/// every scroll container after a window resize), so [`super::use_breakpoint`]
/// tracks the live window width without a dedicated resize event.
///
/// Mount once at the app root in apps that have no other measured width source
/// (Presentation / Spreadsheet — loki-text funnels its editor scroll-container
/// width instead; audit F7a). Requires [`super::use_provide_responsive`] in an
/// ancestor.
///
/// Not an interactive element (zero-size, no pointer targets), so the 44 px
/// touch-target convention does not apply.
#[component]
pub fn AtViewportWidthSensor() -> Element {
    let responsive = use_responsive();
    let mut mounted = use_signal(|| Option::<MountedEvent>::None);
    let measure = move || {
        let Some(evt) = mounted.peek().clone() else {
            return;
        };
        let mut viewport = responsive.viewport;
        spawn(async move {
            if let Ok(rect) = evt.get_client_rect().await {
                let width = rect.size.width as f32;
                let prev = *viewport.peek();
                if width > 0.0 && (prev.inner_width_px - width).abs() > 0.5 {
                    viewport.set(Viewport {
                        inner_width_px: width,
                        ..prev
                    });
                }
            }
        });
    };
    rsx! {
        div {
            style: "width: 100%; height: 0px; overflow: auto;",
            onmounted: move |e| {
                mounted.set(Some(e));
                measure();
            },
            onscroll: move |_| measure(),
        }
    }
}
