// SPDX-License-Identifier: Apache-2.0

//! Blank document creation and the `untitled://` URI scheme.
//!
//! Every new blank document gets a unique `untitled://N` path for its session
//! lifetime.  The path is never persisted or added to the recent-documents list.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::tabs::OpenTab;

/// URI scheme prefix for unsaved blank documents.
pub const UNTITLED_SCHEME: &str = "untitled://";

/// Session-scoped counter — incremented once per [`new_blank_tab`] call.
static UNTITLED_COUNTER: AtomicU32 = AtomicU32::new(1);

/// Returns `true` if `path` refers to an unsaved blank document.
pub fn is_untitled(path: &str) -> bool {
    path.starts_with(UNTITLED_SCHEME)
}

/// Creates a new blank-document tab with a unique `untitled://N` path.
///
/// The tab is pre-marked `is_dirty: true` because the document has never been
/// saved; the title bar will show the unsaved-changes indicator immediately.
///
/// Title convention matches macOS: first blank doc is "Untitled", subsequent
/// ones are "Untitled 2", "Untitled 3", …
pub fn new_blank_tab() -> OpenTab {
    let n = UNTITLED_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = format!("{}{}", UNTITLED_SCHEME, n);
    let title = if n == 1 {
        "Untitled".to_string()
    } else {
        format!("Untitled {}", n)
    };
    OpenTab {
        title,
        path,
        is_dirty: true,
        is_discarded: false,
    }
}
