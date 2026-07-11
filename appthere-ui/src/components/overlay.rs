// SPDX-License-Identifier: Apache-2.0

//! Window-level dismiss backdrop: [`use_provide_backdrop`] + [`AtBackdropHost`]
//! + [`use_backdrop`].
//!
//! `position: fixed` collapses to `absolute` in the current Blitz stack, so a
//! component deep in the tree (e.g. the ribbon's overflow menu) cannot render
//! its own full-viewport click-catcher — an absolute backdrop only spans its
//! nearest positioned ancestor. Instead the app root provides this context and
//! mounts [`AtBackdropHost`] inside its **positioned** root container; any
//! descendant can then request a transparent viewport-spanning backdrop whose
//! click dismisses whatever the requester has open.
//!
//! The popup itself stays wherever the requester rendered it (anchored to its
//! trigger) — only the backdrop is hosted at the window level. The popup must
//! carry a `z-index` above [`BACKDROP_Z_INDEX`] to stay clickable.

use dioxus::prelude::*;

/// z-index of the backdrop click-catcher. Popups that use the backdrop must
/// render above this (the ribbon overflow menu uses 41).
pub const BACKDROP_Z_INDEX: i32 = 40;

/// Context handle for the window-level dismiss backdrop.
#[derive(Clone, Copy)]
pub struct AtBackdropContext {
    /// `Some(on_dismiss)` while a requester has the backdrop up.
    dismiss: Signal<Option<Callback<()>>>,
}

impl AtBackdropContext {
    /// Raises the backdrop; a click anywhere on it calls `on_dismiss` (the
    /// requester closes its own popup state) and lowers the backdrop.
    /// A second `show` replaces the previous requester's callback.
    pub fn show(mut self, on_dismiss: Callback<()>) {
        self.dismiss.set(Some(on_dismiss));
    }

    /// Lowers the backdrop without invoking the dismiss callback (the popup
    /// closed through its own affordance).
    pub fn hide(mut self) {
        if self.dismiss.peek().is_some() {
            self.dismiss.set(None);
        }
    }
}

/// Provides the backdrop context at the application root. Call once, before
/// any component that may request a backdrop; mount [`AtBackdropHost`] inside
/// the app's positioned root container to actually render it.
pub fn use_provide_backdrop() -> AtBackdropContext {
    let dismiss = use_signal(|| None);
    let ctx = AtBackdropContext { dismiss };
    provide_context(ctx);
    ctx
}

/// The backdrop requester handle, or `None` when the app has not wired
/// [`use_provide_backdrop`] (callers degrade to their local dismissal).
#[must_use]
pub fn use_backdrop() -> Option<AtBackdropContext> {
    try_consume_context::<AtBackdropContext>()
}

/// Renders the active backdrop as a transparent, viewport-spanning
/// click-catcher. Mount once, inside the app's `position: relative` root
/// container (typically after the router, so the backdrop paints above the
/// regular content and below the requesting popup's higher `z-index`).
///
/// # Touch target
///
/// The backdrop is itself the (viewport-sized) target; the 44 × 44 px minimum
/// (WCAG 2.5.8) is trivially met while it is shown.
#[component]
pub fn AtBackdropHost() -> Element {
    let ctx = use_context::<AtBackdropContext>();
    let mut dismiss = ctx.dismiss;
    let Some(cb) = *dismiss.read() else {
        return rsx! {};
    };
    rsx! {
        div {
            style: format!(
                "position: absolute; top: 0; left: 0; right: 0; bottom: 0; \
                 z-index: {BACKDROP_Z_INDEX};"
            ),
            onclick: move |_| {
                cb.call(());
                dismiss.set(None);
            },
        }
    }
}
