// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The `untitled-` path scheme for unsaved blank documents.
//!
//! Every new blank document gets a unique `untitled-N` path for its session
//! lifetime. The path is never persisted or added to the recent-documents list.
//!
//! ## Why not `untitled://`?
//!
//! Dioxus Router's `PATH_ASCII_SET` does not encode `:` or `/`, so a path like
//! `"untitled://1"` would serialise to the URL `/editor/untitled://1`. The
//! router splits URLs by `/` and would see four segments instead of the two that
//! `#[route("/editor/:path")]` expects, causing a match failure. `"untitled-1"`
//! is a plain alphanumeric string that is safe as a URL segment.

/// Path prefix for unsaved blank documents.
///
/// Produces paths like `"untitled-1"`, `"untitled-2"` — URL-safe alphanumeric
/// strings with a hyphen separator.
pub const UNTITLED_SCHEME: &str = "untitled-";

/// Returns `true` if `path` refers to an unsaved blank document.
pub fn is_untitled(path: &str) -> bool {
    path.starts_with(UNTITLED_SCHEME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_untitled_paths() {
        assert!(is_untitled("untitled-1"));
        assert!(is_untitled("untitled-42"));
    }

    #[test]
    fn rejects_real_paths() {
        assert!(!is_untitled("/path/to/doc.docx"));
        assert!(!is_untitled("file://token"));
        assert!(!is_untitled(""));
        assert!(!is_untitled("untitle")); // prefix not complete
    }
}
