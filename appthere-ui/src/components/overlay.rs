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
//! trigger) — only the backdrop is hosted at the window level.
//!
//! **That split does not work in this engine, and the ribbon overflow menu is
//! the one consumer still relying on it.** A `z-index` above
//! [`BACKDROP_Z_INDEX`] keeps a popup clickable only if the popup is a sibling
//! of the backdrop; Blitz sorts siblings only, so a popup left in place is
//! hit-tested *after* a root-hosted backdrop regardless of its value. The
//! working arrangement is [`super::popover::AtPopoverHost`], which hosts both
//! layers at the root — see the `TODO(popover-host)` in `ribbon::groups`.

use dioxus::prelude::*;

/// z-index of the backdrop click-catcher.
///
/// A higher `z-index` on the popup only helps when the popup is a **sibling** of
/// this backdrop — i.e. also hosted at the root. Blitz sorts each parent's
/// children among themselves and has no stacking contexts, so a popup rendered in
/// place cannot climb above a root-hosted backdrop no matter what value it
/// carries. See the retraction on `_ROOT_LAYER_BAND_FLOOR`.
pub const BACKDROP_Z_INDEX: i32 = 40;

/// This value is also the floor of the **reserved root-layer band**, enforced by
/// `scripts/check-root-layer-band.py`.
///
/// The design system owns everything from here up: the backdrop at 40, the
/// popover host at 41, the ribbon's overflow menu at 41, and the modal dialogs at
/// 2000+. **Application crates may not enter the band.**
///
/// # The r64 rationale for the band is retracted; the band still earns its place
///
/// r64 said an app-crate value here "competes with a root layer in a stacking
/// context the consumer cannot see". Blitz has **no stacking contexts**: each
/// parent sorts its own `paint_children` by `z_index()` among siblings, and
/// hit-testing walks that same list in reverse. Nothing below the root can
/// outrank a root sibling, at any z-index — so the competition described never
/// happens, and an app-crate 1000 is not dangerous for the stated reason.
///
/// What is true, and is worse: **a root-hosted layer outranks every application
/// surface unconditionally.** That is why the band is reserved — not because app
/// values win, but because they cannot, so a value up here is always a consumer
/// that has misunderstood where its overlay lives. It also means a stale root
/// layer is catastrophic rather than cosmetic, which is what r66 demonstrated.
///
/// The gate duplicates this number as a literal, and says so; if this moves, that
/// moves.
const _ROOT_LAYER_BAND_FLOOR: i32 = BACKDROP_Z_INDEX;

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
