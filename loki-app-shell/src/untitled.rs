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

/// Returns `true` if `path` refers to an unsaved (untitled) document — blank,
/// created from a bundled template, or imported from an external template file.
pub fn is_untitled(path: &str) -> bool {
    path.starts_with(UNTITLED_SCHEME)
}

/// Marker separating the counter from a bundled-template id in an untitled path.
const MARKER_TPL: &str = "-tpl-";
/// Marker separating the counter from an imported file's token in an untitled path.
const MARKER_IMP: &str = "-imp-";

/// How a new (untitled) document's initial content is produced.
///
/// Encoded in the path *after* the counter so [`is_untitled`] stays true and the
/// path remains a single URL-safe segment that survives router round-trips and
/// session restore (the content is always reproducible from the path alone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewDocSource {
    /// Empty document with the built-in heading styles.
    Blank,
    /// A bundled template identified by its short id (e.g. `"apa"`).
    Template(String),
    /// An external file (a serialized file token) opened as a fresh, detached
    /// document — saving prompts Save As rather than overwriting the source.
    Import(String),
}

/// Builds the untitled path for bundled template `id` with counter `n`.
#[must_use]
pub fn template_path(n: u32, id: &str) -> String {
    format!("{UNTITLED_SCHEME}{n}{MARKER_TPL}{id}")
}

/// Builds the untitled path that imports external `token` as a fresh document.
#[must_use]
pub fn import_path(n: u32, token: &str) -> String {
    format!("{UNTITLED_SCHEME}{n}{MARKER_IMP}{token}")
}

/// Parses how to build a new document from its untitled `path`.
///
/// Returns `None` when `path` is not an untitled path (i.e. a real file path).
/// The counter is the leading run of digits, so the source marker — when present
/// — begins immediately after it, making the parse unambiguous regardless of the
/// payload's own contents (an imported token may itself contain hyphens).
#[must_use]
pub fn parse_new_doc_source(path: &str) -> Option<NewDocSource> {
    let rest = path.strip_prefix(UNTITLED_SCHEME)?;
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let after = &rest[digits_end..];
    if let Some(id) = after.strip_prefix(MARKER_TPL) {
        return Some(NewDocSource::Template(id.to_string()));
    }
    if let Some(token) = after.strip_prefix(MARKER_IMP) {
        return Some(NewDocSource::Import(token.to_string()));
    }
    Some(NewDocSource::Blank)
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

    #[test]
    fn parses_blank_source() {
        assert_eq!(
            parse_new_doc_source("untitled-1"),
            Some(NewDocSource::Blank)
        );
        assert_eq!(
            parse_new_doc_source("untitled-42"),
            Some(NewDocSource::Blank)
        );
        assert_eq!(parse_new_doc_source("/real/file.docx"), None);
    }

    #[test]
    fn round_trips_template_and_import_paths() {
        let tp = template_path(3, "apa");
        assert!(is_untitled(&tp));
        assert_eq!(
            parse_new_doc_source(&tp),
            Some(NewDocSource::Template("apa".into()))
        );

        // An imported token may itself contain hyphens — parsing anchors on the
        // marker right after the counter, so the whole token is recovered.
        let token = "file-token-abc-123";
        let ip = import_path(7, token);
        assert!(is_untitled(&ip));
        assert_eq!(
            parse_new_doc_source(&ip),
            Some(NewDocSource::Import(token.into()))
        );
    }
}
