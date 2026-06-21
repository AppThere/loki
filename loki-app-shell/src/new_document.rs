// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Blank-document tab creation shared across the Loki editor shells.
//!
//! Every new blank document gets a unique `untitled-N` path (see [`crate::untitled`])
//! for its session lifetime. The path is never persisted or added to the
//! recent-documents list.

use std::sync::atomic::{AtomicU32, Ordering};

use loki_i18n::fl;

use crate::tabs::OpenTab;
use crate::untitled::UNTITLED_SCHEME;

/// Session-scoped counter — incremented once per [`new_blank_tab`] call.
///
/// Each Loki application is a separate process, so although this static is
/// compiled into a shared crate every running binary has its own instance.
static UNTITLED_COUNTER: AtomicU32 = AtomicU32::new(1);

/// Creates a new blank-document tab with a unique `untitled-N` path.
///
/// The tab is pre-marked `is_dirty: true` because the document has never been
/// saved; the title bar will show the unsaved-changes indicator immediately.
///
/// Title convention matches macOS: the first blank document is "Untitled",
/// subsequent ones are "Untitled 2", "Untitled 3", …
pub fn new_blank_tab() -> OpenTab {
    let n = UNTITLED_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = format!("{}{}", UNTITLED_SCHEME, n);
    let title = if n == 1 {
        fl!("editor-untitled")
    } else {
        fl!("editor-untitled-n", n = n as i64)
    };
    OpenTab {
        title,
        path,
        is_dirty: true,
        is_discarded: false,
    }
}
