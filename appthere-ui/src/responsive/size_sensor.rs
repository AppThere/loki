// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`AtWindowSizeSensor`] — reports the app root's logical size to the caller
//! (e.g. for persisting window dimensions across sessions).

use dioxus::prelude::*;

/// Invisible probes that measure the app root's width **and** height: once at
/// mount, and again on every shell `resync_scroll_geometry` tick (the blitz
/// shell re-emits `onscroll` to every scroll container after a window resize),
/// reporting `(width, height)` in logical pixels whenever either changes by
/// more than half a pixel.
///
/// Mount inside the app's root element (which must be `position: relative` and
/// span the window, as the standard shell root does). The probes are
/// absolutely positioned and zero-area — one full-width × zero-height, one
/// zero-width × full-height — so they never affect flow or intercept input.
///
/// Not an interactive element (zero-size, no pointer targets), so the 44 px
/// touch-target convention does not apply.
#[component]
pub fn AtWindowSizeSensor(
    /// Called with the root's `(width, height)` in logical pixels on mount and
    /// after each observed change.
    on_size: EventHandler<(f64, f64)>,
) -> Element {
    let mut width_mounted = use_signal(|| Option::<MountedEvent>::None);
    let mut height_mounted = use_signal(|| Option::<MountedEvent>::None);
    let last = use_signal(|| (0.0_f64, 0.0_f64));

    let measure = move || {
        let (Some(w_evt), Some(h_evt)) =
            (width_mounted.peek().clone(), height_mounted.peek().clone())
        else {
            return;
        };
        let mut last = last;
        spawn(async move {
            let (Ok(w_rect), Ok(h_rect)) =
                (w_evt.get_client_rect().await, h_evt.get_client_rect().await)
            else {
                return;
            };
            let size = (w_rect.size.width, h_rect.size.height);
            let (pw, ph) = *last.peek();
            if size.0 > 0.0
                && size.1 > 0.0
                && ((size.0 - pw).abs() > 0.5 || (size.1 - ph).abs() > 0.5)
            {
                last.set(size);
                on_size.call(size);
            }
        });
    };

    rsx! {
        div {
            style: "position: absolute; top: 0; left: 0; width: 100%; height: 0px; overflow: auto;",
            onmounted: move |e| {
                width_mounted.set(Some(e));
                measure();
            },
            onscroll: move |_| measure(),
        }
        div {
            style: "position: absolute; top: 0; left: 0; width: 0px; height: 100%; overflow: auto;",
            onmounted: move |e| {
                height_mounted.set(Some(e));
                measure();
            },
            onscroll: move |_| measure(),
        }
    }
}
