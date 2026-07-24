// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Picker-mediated file access model (macro spec §5.3, Phase 7B).
//!
//! There is **no path-addressed file API** (`Open … For`, `FileSystemObject`,
//! `Dir`, `Kill`, … do not exist — spec §5.3, threat T3). A macro that wants a
//! file calls `Application.OpenFileForReading(filter)`, which raises the OS
//! picker; the user's pick *is* the grant, scoped to that handle for that run.
//! These are the plain data types that cross the [`crate::exec::MacroBackend`]
//! seam — the app performs the actual pick + read.

/// A file the user chose through the OS picker. The pick is the grant, so this
/// carries only what the macro may read: the chosen path (so a script can tell
/// which file it got) and the bytes. A macro cannot construct one for a path it
/// names — there is no such API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickedFile {
    /// The absolute path the user picked (display / echo only).
    pub path: String,
    /// The file's bytes, as read by the app at pick time.
    pub bytes: Vec<u8>,
}

impl PickedFile {
    /// The contents decoded as UTF-8, lossily (macros read text).
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    /// The byte length of the file.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.bytes.len()
    }
}

/// A file-type filter the macro passed to `OpenFileForReading` (e.g. `"*.txt"`).
/// Advisory — it shapes the picker's file-type list; the user's actual pick is
/// the authority. Empty `extensions` means "any file".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFilter {
    /// Bare, lower-cased extensions without the leading `*.`/`.` (e.g. `["txt",
    /// "csv"]`). Empty = any file.
    pub extensions: Vec<String>,
}

impl FileFilter {
    /// Parse a VBA-style filter argument into bare extensions, tolerating the
    /// common shapes a macro passes: `"*.txt"`, `"*.txt;*.csv"`, `"txt,csv"`, and
    /// the `GetOpenFilename` `"Text Files (*.txt),*.txt"` description/pattern
    /// form. Keeps only the extension tokens (lower-cased, de-duplicated, order
    /// preserved); a wildcard-only or empty filter yields "any file".
    #[must_use]
    pub fn parse(arg: &str) -> Self {
        let mut extensions: Vec<String> = Vec::new();
        let mut push = |ext: String| {
            if !ext.is_empty() && ext != "*" && !extensions.contains(&ext) {
                extensions.push(ext);
            }
        };

        // First pass: every `*.ext` pattern anywhere in the string.
        let mut found_star = false;
        for (idx, _) in arg.match_indices("*.") {
            found_star = true;
            let rest = &arg[idx + 2..];
            let ext: String = rest
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .flat_map(char::to_lowercase)
                .collect();
            push(ext);
        }

        // If no `*.ext` patterns, treat comma/semicolon-separated bare alnum
        // tokens as extensions (`"txt,csv"`).
        if !found_star {
            for token in arg.split([',', ';']) {
                let token = token.trim().trim_start_matches('.');
                if !token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric()) {
                    push(token.to_ascii_lowercase());
                }
            }
        }

        Self { extensions }
    }
}

/// Why a picker-mediated file **write** could not complete (macro spec §5.3,
/// Phase 7B). The user's pick of a save target was the consent; a failure here
/// surfaces to the macro as a trappable error it can `On Error` around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileWriteError {
    /// The backend performs no writes at all — the safe default for a
    /// non-interactive context (UDF, headless, tests).
    Refused,
    /// The OS write failed (permission denied, disk full, …). The message is for
    /// display only; it never carries a privileged object.
    Io(String),
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
