// SPDX-License-Identifier: Apache-2.0

//! PATCH(loki): Android soft-keyboard visibility bridge.
//!
//! On a `NativeActivity` the OS never tells the app when the *user* dismisses
//! (or re-summons) the soft keyboard — the back button, the swipe-down gesture,
//! and the keyboard's own hide key are all invisible to winit / Blitz / Dioxus,
//! and the surface is not resized. Without a signal the app keeps the bottom
//! safe area reserved for a keyboard that is gone (dead space below the
//! document), and the `WindowState::ime_active` mirror goes stale so the next
//! caret tap needlessly takes the `force_show` re-summon path.
//!
//! `loki-file-access` installs a decor-view `OnApplyWindowInsetsListener` that
//! fires on every IME transition and calls [`notify_ime_visibility_changed`].
//! That callback arrives on the Android UI thread; this module hops it onto the
//! winit event loop by storing the new visibility and waking the loop with a
//! `Poll`. [`take_pending`] is then drained in `WindowState::poll`, which syncs
//! `ime_active` and arms the existing IME-settle re-sync so the bottom safe
//! area converges to the new keyboard height (0 on a collapse).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

use crate::event::BlitzShellEvent;

/// Target woken by the `Poll` sent on an IME-visibility change. `None` until a
/// window registers itself via [`register`] during `View::init`.
static TARGET: Mutex<Option<(EventLoopProxy<BlitzShellEvent>, WindowId)>> = Mutex::new(None);

/// Pending IME-visibility change awaiting drain by `poll`:
/// `0` = none, `1` = hidden, `2` = visible.
static PENDING: AtomicU8 = AtomicU8::new(0);

/// Record the event-loop proxy + window to wake when the platform reports an
/// IME-visibility change. Called once when the window is created.
pub(crate) fn register(proxy: EventLoopProxy<BlitzShellEvent>, window_id: WindowId) {
    if let Ok(mut guard) = TARGET.lock() {
        *guard = Some((proxy, window_id));
    }
}

/// Record a platform IME-visibility change and wake the event loop so `poll`
/// can react.
///
/// Safe to call from any thread — it is invoked from the Android UI thread by
/// the JNI inset listener. A no-op (aside from storing the pending value) if no
/// window has registered yet.
pub fn notify_ime_visibility_changed(visible: bool) {
    PENDING.store(if visible { 2 } else { 1 }, Ordering::SeqCst);
    if let Ok(guard) = TARGET.lock()
        && let Some((proxy, window_id)) = guard.as_ref()
    {
        let _ = proxy.send_event(BlitzShellEvent::Poll {
            window_id: *window_id,
        });
    }
}

/// Drain a pending IME-visibility change, if any, returning the new visibility.
pub(crate) fn take_pending() -> Option<bool> {
    match PENDING.swap(0, Ordering::SeqCst) {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}
