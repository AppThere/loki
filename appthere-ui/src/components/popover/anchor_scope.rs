// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tying an open popover's lifetime to its consumer's (Spec 08 T4.1, r66).
//!
//! # The primitive named this hazard and then did not wire it
//!
//! [`super::interaction::DismissCause::AnchorUnmounted`] has documented, since
//! the primitive was written, that root hosting decouples the popup's lifetime
//! from its anchor's and that **the anchor's own cleanup must close it**. It had
//! no caller. A decision nothing calls is indistinguishable from one nobody made,
//! and this is what it cost:
//!
//! `SpellPopover` is mounted behind `if spell_menu.read().is_some()`. Choosing a
//! suggestion set that signal to `None`; the consumer unmounted; and its
//! `use_effect` — the only thing that ever called `dismiss` — went with it. The
//! host's `open` stayed `Some` indefinitely. The *menu* still disappeared,
//! because the content closure reads the same signal and renders nothing, so what
//! remained was the backdrop alone: transparent, window-sized, at the app root.
//! From that moment every click in the application hit it, and clicking it ran
//! `on_dismiss`, which set an already-`None` signal and changed nothing. The
//! editor, the scrollbar and the tab bar were all dead, with nothing on screen to
//! explain why.
//!
//! # Why this is a hook and not a line in each consumer
//!
//! Four consumers are planned. "Remember to dismiss on unmount" is exactly the
//! kind of obligation three of them will meet and one will not, and the one that
//! does not produces a dead application rather than a visible glitch — so the
//! remedy has to make the wrong thing unavailable rather than documented
//! (L08-043).
//!
//! [`use_popover_anchor`] installs the cleanup as a condition of getting the
//! opener. [`AtPopoverContext::open_resolved`] is `pub(crate)`, so a consumer
//! cannot reach it any other way: opening a popover has, by construction, already
//! registered the thing that closes it.

use dioxus::prelude::*;

use super::component::{use_popover, AtPopoverContext, PopoverRequest};
use super::wiring::{dismiss_on_unmount, PopoverId};

impl AtPopoverContext {
    /// Closes the popover **only if `id` is the one currently open**.
    ///
    /// See [`dismiss_on_unmount`] for why it is keyed rather than unconditional:
    /// the singleton rule means another popover may have replaced this one, and
    /// closing that one instead would be an intermittent vanishing menu.
    ///
    /// Reads and writes fallibly. This runs from a drop handler, which on
    /// application teardown can execute after the root scope's signals are gone;
    /// `peek`/`set` would panic there, turning a clean exit into a crash on quit.
    /// It is also `peek` rather than `read` on purpose — a drop handler is not a
    /// reactive context and must not subscribe.
    pub fn dismiss_if_open(mut self, id: PopoverId) {
        let Ok(open) = self.open.try_peek() else {
            return; // runtime is tearing down; nothing left to dismiss
        };
        let currently_open = open.as_ref().map(|request| request.id);
        drop(open);
        if !dismiss_on_unmount(currently_open, id) {
            return;
        }
        if let Ok(mut slot) = self.open.try_write() {
            *slot = None;
        }
        if let Ok(mut slot) = self.resolved.try_write() {
            *slot = None;
        }
    }
}

/// A consumer's handle on the popover host, bound to the id it opens under.
///
/// The only way to open a popover from outside this crate, and it exists only
/// from [`use_popover_anchor`] — so the unmount cleanup is not something a
/// consumer can forget to install.
#[derive(Clone, Copy)]
pub struct PopoverAnchor {
    ctx: AtPopoverContext,
    id: PopoverId,
}

impl PopoverAnchor {
    /// Opens `request`, resolving its placement against the measured window.
    ///
    /// `request.id` is **stamped with this anchor's id**. A consumer that
    /// registered under one id and opened under another would install a cleanup
    /// that never fires, which is precisely the defect this type exists to close;
    /// stamping makes the two agree by construction rather than by care.
    pub fn open(
        self,
        request: PopoverRequest,
        window: Option<(f64, f64)>,
        insets: crate::SafeAreaInsets,
    ) {
        let mut request = request;
        request.id = self.id;
        self.ctx.open_resolved(request, window, insets);
    }

    /// Closes this popover, and only this one.
    ///
    /// Keyed on the anchor's id — see [`dismiss_on_unmount`]. A consumer whose
    /// popover was already replaced under the singleton rule closes nothing here,
    /// rather than closing the replacement.
    pub fn dismiss(self) {
        self.ctx.dismiss_if_open(self.id);
    }
}

/// A [`PopoverAnchor`] with this consumer's unmount cleanup already installed.
///
/// `id` is the [`PopoverId`] this component opens under; when the component
/// unmounts, whatever it has open under that id is closed.
///
/// Returns `None` when the application has not called
/// [`super::component::use_provide_popover`], on the same degrade-quietly rule as
/// [`use_popover`].
///
/// # Touch target
///
/// Not applicable — this is a hook and renders nothing.
#[must_use]
pub fn use_popover_anchor(id: PopoverId) -> Option<PopoverAnchor> {
    let ctx = use_popover();
    // Unconditional, as every hook must be: `ctx` is captured by value and the
    // `None` case is handled inside, rather than by skipping the hook.
    use_drop(move || {
        if let Some(ctx) = ctx {
            ctx.dismiss_if_open(id);
        }
    });
    ctx.map(|ctx| PopoverAnchor { ctx, id })
}
