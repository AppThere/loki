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

/// The window's measured logical size, shared with anything that places against
/// the window rather than against a container.
///
/// # Why this exists when the sensor already did
///
/// [`AtWindowSizeSensor`] has reported `(width, height)` on mount and resize
/// since Spec 04 — and its only consumer persisted the geometry to disk. So the
/// measurement existed and the *value* did not reach anyone, which is
/// indistinguishable from not measuring it: wiring T4.1's popover host, the
/// first look for a window height found `ScrollMetrics` (the editor scroll
/// container) and `responsive::Viewport::inner_width_px` (that container's
/// width, no height) and concluded the workspace had none.
///
/// It had one. Deriving a window height from a container plus known chrome is
/// exactly the ~41px class of error T4.1 exists to remove, so the fix is to
/// publish what is already measured rather than to measure it again.
#[derive(Clone, Copy)]
pub struct AtWindowSizeContext {
    /// Logical `(width, height)`; `(0, 0)` until the first measurement lands.
    pub size: Signal<(f64, f64)>,
}

/// Provides [`AtWindowSizeContext`] at the app root. Returns the signal so the
/// root can feed it from [`AtWindowSizeSensor`].
#[must_use]
pub fn use_provide_window_size() -> Signal<(f64, f64)> {
    let size = use_signal(|| (0.0_f64, 0.0_f64));
    use_context_provider(|| AtWindowSizeContext { size });
    size
}

/// The measured window size, or `None` where no root provided it.
///
/// `None` and `Some((0.0, 0.0))` are different answers and both are possible:
/// the first means nobody is measuring, the second that measurement has not
/// arrived yet. A consumer that treats them alike will place against a
/// zero-sized viewport on the first frame.
#[must_use]
pub fn use_window_size() -> Option<(f64, f64)> {
    window_size_signal().map(|s| *s.read())
}

/// The window-size **signal**, for a consumer that must read it from inside an
/// effect rather than at render time.
///
/// # Why a second accessor rather than one that returns a value
///
/// [`use_window_size`] reads the signal where it is called, so a component gets
/// the value and a subscription — correct for rendering. An **effect** captures
/// that value and is therefore never re-run by a resize: the read happened
/// outside the closure, so the effect subscribed to nothing.
///
/// The popover's anchor driver (Spec 08 D-15) is exactly that consumer — a
/// resize is one of its two change sources, and it would silently respond only
/// to the other. Handing back the signal lets the effect read it, and read is
/// what creates the subscription.
///
/// Deliberately **not** a `use_` hook: it calls no hook, so it is legal inside an
/// effect closure, which is the whole point. Naming it `use_window_size_signal`
/// would assert a hook contract it does not have and cannot honour there
/// (L08-031).
#[must_use]
pub fn window_size_signal() -> Option<Signal<(f64, f64)>> {
    try_consume_context::<AtWindowSizeContext>().map(|c| c.size)
}
