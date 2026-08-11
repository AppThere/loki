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
use loki_file_access::FileAccessToken;

use super::Route;
use super::editor::{DocumentFormat, detect_format};
use super::home_util::{opens_as_detached_copy, push_new_tab, push_or_switch_tab};
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
pub(super) fn tokenize_candidates(paths: Vec<String>) -> Vec<StartupCandidate> {
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
