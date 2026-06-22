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

/// Creates a new tab seeded from bundled template `template_id`.
///
/// Like a blank tab the document is untitled (its `untitled-N-tpl-…` path keeps
/// it out of recents and forces Save As), but `load_document` reconstructs the
/// template's content from the path. `title` is the template's display name.
pub fn new_template_tab(template_id: &str, title: String) -> OpenTab {
    let n = UNTITLED_COUNTER.fetch_add(1, Ordering::Relaxed);
    OpenTab {
        title,
        path: crate::untitled::template_path(n, template_id),
        is_dirty: true,
        is_discarded: false,
    }
}

/// Creates a new tab that imports external `token` as a fresh, detached document
/// (the template-file open flow). Untitled, so saving prompts Save As rather
/// than overwriting the source file. `title` is typically the file's stem.
pub fn new_import_tab(token: &str, title: String) -> OpenTab {
    let n = UNTITLED_COUNTER.fetch_add(1, Ordering::Relaxed);
    OpenTab {
        title,
        path: crate::untitled::import_path(n, token),
        is_dirty: true,
        is_discarded: false,
    }
}
