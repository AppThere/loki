// SPDX-License-Identifier: Apache-2.0

//! Turns launch-argument paths into open tabs (§13 layer 2). The [`Shell`]
//! layout runs [`seed_pending`] in an effect: the stash drains once, each
//! path becomes a [`FileAccessToken`] (via the vendored `from_path` — see
//! docs/patches.md), and the app lands on the first opened document's editor
//! instead of Home.
//!
//! [`Shell`]: super::shell::Shell

use dioxus::prelude::*;
use dioxus_router::Navigator;

use super::Route;
use super::home_util::{push_new_tab, push_or_switch_tab};
use crate::new_document::new_import_tab;
use crate::recent_documents::RecentDocuments;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

/// One launch path resolved into something the tab list can open.
pub(super) struct StartupCandidate {
    /// The serialized token (the editor route's `path`).
    pub serialized: String,
    /// Whether the file opens as a detached copy (templates and import-only
    /// formats — same predicate as the picker flow).
    pub detached: bool,
}

/// Filters raw launch paths into openable candidates: the file must exist
/// (token construction canonicalizes) and carry a supported extension.
/// Unopenable arguments are skipped with a log line — at launch there is no
/// surface to report to yet, and refusing to boot over a typo would be worse.
///
/// # Desktop only
///
/// The cfg mirrors `FileAccessToken::from_path`'s own (see docs/patches.md) —
/// **that** is the source of truth for which platforms can turn a bare path
/// into a token, and this list must follow it if it changes. Elsewhere, file
/// access arrives as a platform grant object (Android content URIs, Apple
/// security-scoped bookmarks) that a path cannot represent.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn tokenize_candidates(paths: Vec<String>) -> Vec<StartupCandidate> {
    // Imported here rather than at module scope: these are the *only* uses, so
    // at module scope they would be unused imports on every other platform,
    // needing a second copy of the cfg above to silence.
    use super::editor::{DocumentFormat, detect_format};
    use super::home_util::opens_as_detached_copy;
    use loki_file_access::FileAccessToken;

    paths
        .into_iter()
        .filter_map(|p| {
            let token = match FileAccessToken::from_path(&p) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(path = %p, error = %e, "startup file not openable");
                    return None;
                }
            };
            if matches!(detect_format(&token), DocumentFormat::Unsupported(_)) {
                tracing::warn!(path = %p, "startup file has an unsupported format");
                return None;
            }
            Some(StartupCandidate {
                detached: opens_as_detached_copy(token.display_name()),
                serialized: token.serialize(),
            })
        })
        .collect()
}

/// No launch path can become a token on this platform, so nothing is seeded.
///
/// This is a **deferral, not a gap being papered over**: the stash it would
/// drain is filled by `main` from argv, and the Android entry point is the
/// `cdylib`'s `android_main`, which has no argv and never populates it — so
/// this arm returns empty for a stash that is already always empty. Android's
/// route to the same feature is the VIEW intent-filter plus a content-URI
/// token, which is §13 layer 3 and unbuilt.
///
/// TODO(startup-android-intent): seed from the launch Intent's data URI once
/// the manifest declares the VIEW filter and the vendored patch grows
/// `from_content_uri`.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(super) fn tokenize_candidates(_paths: Vec<String>) -> Vec<StartupCandidate> {
    Vec::new()
}

/// Drains the startup stash and opens each candidate, navigating to the
/// first. Called from the shell's effect; the drain-once stash makes it
/// idempotent across re-renders.
pub(super) fn seed_pending(
    navigator: Navigator,
    tabs: Signal<Vec<OpenTab>>,
    active_tab: Signal<usize>,
    mut recent_docs: Signal<RecentDocuments>,
) {
    let candidates = tokenize_candidates(crate::startup_files::take_paths());
    let mut first_route: Option<String> = None;
    for candidate in candidates {
        let title = display_title_from_path(&candidate.serialized);
        let path = if candidate.detached {
            // Same posture as the picker: a template or import-only file
            // opens as a fresh untitled document (save prompts Save As) and
            // stays out of recents.
            push_new_tab(
                tabs,
                active_tab,
                new_import_tab(&candidate.serialized, title),
            )
        } else {
            push_or_switch_tab(tabs, active_tab, candidate.serialized.clone());
            recent_docs
                .write()
                .record(candidate.serialized.clone(), title);
            recent_docs.read().save();
            candidate.serialized
        };
        first_route.get_or_insert(path);
    }
    if let Some(path) = first_route {
        navigator.push(Route::Editor { path });
    }
}

#[cfg(test)]
#[path = "startup_open_tests.rs"]
mod tests;
