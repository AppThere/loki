// SPDX-License-Identifier: Apache-2.0

//! Files handed to the process at launch (§13 layer 2): `main` stashes raw
//! argv paths here before the UI starts; the router shell drains the stash
//! once contexts exist and turns them into open tabs
//! (`routes::startup_open`). A process-global because it crosses the
//! `dioxus::native::launch` boundary, which takes no user payload.

use std::sync::{Mutex, OnceLock};

static STARTUP_PATHS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn cell() -> &'static Mutex<Vec<String>> {
    STARTUP_PATHS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Stashes launch arguments that look like file paths (everything that is not
/// a `-`-prefixed flag). Validation — existence, supported format — happens
/// at drain time, where the failure can be reported against a UI.
pub fn stash_from_args(args: impl Iterator<Item = String>) {
    let paths: Vec<String> = args.filter(|a| !a.starts_with('-')).collect();
    if paths.is_empty() {
        return;
    }
    if let Ok(mut stash) = cell().lock() {
        stash.extend(paths);
    }
}

/// Drains every stashed path. Empty on every call after the first non-empty
/// drain — the seeding effect runs on each shell render and must be
/// idempotent.
pub(crate) fn take_paths() -> Vec<String> {
    cell()
        .lock()
        .map(|mut s| std::mem::take(&mut *s))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{stash_from_args, take_paths};

    #[test]
    fn flags_are_filtered_and_the_drain_is_once() {
        stash_from_args(
            [
                "--flag".to_string(),
                "/tmp/a.docx".to_string(),
                "-v".to_string(),
            ]
            .into_iter(),
        );
        let drained = take_paths();
        assert!(drained.contains(&"/tmp/a.docx".to_string()));
        assert!(!drained.iter().any(|p| p.starts_with('-')));
        assert!(take_paths().is_empty(), "second drain must be empty");
    }
}
