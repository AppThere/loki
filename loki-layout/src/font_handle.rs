// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Warm-up-aware shared handle to [`FontResources`].
//!
//! [`FontResources::new`] performs a full system-font scan (DirectWrite
//! enumeration on Windows), which can take long enough to freeze a UI frame.
//! [`SharedFontResources`] lets an application start that scan on a background
//! thread at launch ([`SharedFontResources::warm_up`]) and hand out a cheap
//! cloneable handle immediately. Consumers either block until the scan is done
//! ([`SharedFontResources::lock`], for worker threads) or observe readiness
//! without blocking ([`SharedFontResources::try_lock`], for render paths).

use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};

use crate::font::FontResources;

/// Cheap cloneable handle to a lazily warmed, shared [`FontResources`].
///
/// All clones refer to the same underlying font context; the system-font scan
/// is paid at most once per handle family.
#[derive(Clone)]
pub struct SharedFontResources {
    inner: Arc<Inner>,
}

struct Inner {
    /// The built font context. Empty until the warm-up finishes (or the first
    /// blocking `lock` builds it inline as a fallback).
    slot: OnceLock<Mutex<FontResources>>,
    /// Warm-up completion flag + condvar for blocking waiters. Set even if the
    /// warm-up thread dies without filling `slot`, so waiters never hang.
    done: Mutex<bool>,
    ready: Condvar,
}

/// Marks the warm-up as finished when dropped — including on unwind — so a
/// panicking warm-up thread wakes waiters (which then build inline) instead of
/// deadlocking them.
struct DoneOnDrop(Arc<Inner>);

impl Drop for DoneOnDrop {
    fn drop(&mut self) {
        let mut done = self.0.done.lock().unwrap_or_else(|e| e.into_inner());
        *done = true;
        self.0.ready.notify_all();
    }
}

impl SharedFontResources {
    fn empty() -> Self {
        Self {
            inner: Arc::new(Inner {
                slot: OnceLock::new(),
                done: Mutex::new(false),
                ready: Condvar::new(),
            }),
        }
    }

    /// Wraps an already-built [`FontResources`]; the handle is ready at once.
    pub fn new_ready(resources: FontResources) -> Self {
        let handle = Self::empty();
        let _ = handle.inner.slot.set(Mutex::new(resources));
        drop(DoneOnDrop(Arc::clone(&handle.inner)));
        handle
    }

    /// Starts building [`FontResources`] (system-font scan + family-index
    /// population) on a background thread and returns immediately.
    ///
    /// If the thread cannot be spawned, the scan runs inline as a fallback.
    pub fn warm_up() -> Self {
        let handle = Self::empty();
        let inner = Arc::clone(&handle.inner);
        let spawned = std::thread::Builder::new()
            .name("loki-font-warmup".into())
            .spawn(move || {
                let _done = DoneOnDrop(Arc::clone(&inner));
                let mut resources = FontResources::new();
                // Fontique populates its family-name index lazily; touch it here
                // so the style editor's font enumeration is also prepaid.
                let _ = resources.available_font_families();
                let _ = inner.slot.set(Mutex::new(resources));
            });
        if spawned.is_err() {
            let _ = handle.inner.slot.set(Mutex::new(FontResources::new()));
            drop(DoneOnDrop(Arc::clone(&handle.inner)));
        }
        handle
    }

    /// Locks the shared font context, blocking until the warm-up has finished.
    ///
    /// Intended for worker threads (layout, export) and post-load paths where
    /// the scan is long since complete. If the warm-up thread died without
    /// producing a context, one is built inline here.
    pub fn lock(&self) -> MutexGuard<'_, FontResources> {
        if self.inner.slot.get().is_none() {
            let mut done = self.inner.done.lock().unwrap_or_else(|e| e.into_inner());
            while !*done {
                done = self
                    .inner
                    .ready
                    .wait(done)
                    .unwrap_or_else(|e| e.into_inner());
            }
        }
        self.inner
            .slot
            .get_or_init(|| Mutex::new(FontResources::new()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Non-blocking lock: returns `None` while the warm-up is still running
    /// *or* while another thread (e.g. a layout worker) holds the lock.
    ///
    /// Intended for render paths that must never stall a frame; callers treat
    /// `None` as "no font information this frame" and pick the data up on a
    /// later render.
    pub fn try_lock(&self) -> Option<MutexGuard<'_, FontResources>> {
        self.inner.slot.get()?.try_lock().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ready_is_immediately_lockable() {
        let handle = SharedFontResources::new_ready(FontResources::new());
        assert!(handle.try_lock().is_some());
        let _guard = handle.lock();
    }

    #[test]
    fn warm_up_lock_blocks_until_built_and_clones_share() {
        let handle = SharedFontResources::warm_up();
        let clone = handle.clone();
        // Blocking lock from a worker thread must rendezvous with the warm-up.
        // A family that certainly exists on no platform, so resolving it always
        // records an entry in `substitutions`.
        const MISSING: &str = "Loki Test Nonexistent Family";
        let joined = std::thread::spawn(move || {
            let mut fr = clone.lock();
            fr.resolve_font_name(MISSING)
        })
        .join()
        .expect("worker thread panicked");
        assert!(!joined.is_empty());
        // The substitution recorded on the worker is visible through the
        // original handle: both point at the same FontResources.
        let fr = handle.lock();
        assert!(fr.substitutions.contains_key(MISSING));
    }

    #[test]
    fn try_lock_yields_while_another_thread_holds_the_guard() {
        let handle = SharedFontResources::new_ready(FontResources::new());
        let guard = handle.lock();
        assert!(handle.try_lock().is_none());
        drop(guard);
        assert!(handle.try_lock().is_some());
    }
}
