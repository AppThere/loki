// SPDX-License-Identifier: Apache-2.0

//! Blank document creation and the `untitled-` path prefix.
//!
//! Every new blank document gets a unique `untitled-N` path for its session
//! lifetime.  The path is never persisted or added to the recent-documents list.
//!
//! ## Why not `untitled://`?
//!
//! Dioxus Router's PATH_ASCII_SET does not encode `:` or `/`, so a path like
//! `"untitled://1"` would serialise to the URL `/editor/untitled://1`.  The
//! router splits URLs by `/` and would see four segments instead of the two
//! that `#[route("/editor/:path")]` expects, causing a match failure.
//! `"untitled-1"` is a plain alphanumeric string that is safe as a URL segment.

use std::sync::atomic::{AtomicU32, Ordering};

use loki_i18n::fl;

use crate::tabs::OpenTab;

/// Path prefix for unsaved blank documents.
///
/// Produces paths like `"untitled-1"`, `"untitled-2"` — URL-safe alphanumeric
/// strings with a hyphen separator.
pub const UNTITLED_SCHEME: &str = "untitled-";

/// Session-scoped counter — incremented once per [`new_blank_tab`] call.
static UNTITLED_COUNTER: AtomicU32 = AtomicU32::new(1);

/// Returns `true` if `path` refers to an unsaved blank document.
pub fn is_untitled(path: &str) -> bool {
    path.starts_with(UNTITLED_SCHEME)
}

/// Creates a new blank-document tab with a unique `untitled-N` path.
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
