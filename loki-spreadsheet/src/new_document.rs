// SPDX-License-Identifier: Apache-2.0

//! Blank spreadsheet creation and the `untitled-` path prefix.

use crate::tabs::OpenTab;
use loki_i18n::fl;
use std::sync::atomic::{AtomicU32, Ordering};

pub const UNTITLED_SCHEME: &str = "untitled-";
static UNTITLED_COUNTER: AtomicU32 = AtomicU32::new(1);

pub fn is_untitled(path: &str) -> bool {
    path.starts_with(UNTITLED_SCHEME)
}

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
