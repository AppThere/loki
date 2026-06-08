// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Application-level utility functions for `loki-spreadsheet`.

use loki_file_access::FileAccessToken;

// ── display_title_from_path ───────────────────────────────────────────────────

/// Derive a human-readable document title from a raw route `path` segment.
///
/// The `path` parameter is a URL-safe base64 token produced by
/// [`FileAccessToken::serialize`].  The priority order is:
///
/// 1. **Token display name** — [`FileAccessToken::deserialize`] extracts the
///    platform-reported filename (e.g. `"Q1 Report.xlsx"`); the extension is
///    stripped and the stem is title-cased.
/// 2. **Filename stem** — when the token cannot be decoded (malformed path,
///    future format change), the last path component is extracted with
///    underscores and hyphens replaced by spaces, then title-cased.
/// 3. **Fallback** — `"Untitled Document"` when neither produces a usable
///    string.
pub fn display_title_from_path(path: &str) -> String {
    if path.is_empty() {
        return "Untitled Document".to_string();
    }

    // Primary path: the route param is a serialised FileAccessToken.
    // Deserialising it gives us the exact display name the platform reported.
    if let Ok(token) = FileAccessToken::deserialize(path) {
        let title = format_stem(strip_extension(token.display_name()));
        if !title.is_empty() {
            return title;
        }
    }

    // Fallback: treat `path` as a raw file path (percent-decoded).
    // Handles unusual cases where someone constructs the route param directly.
    let decoded = percent_decode(path);
    let filename = decoded
        .split(['/', '\\'])
        .rfind(|s| !s.is_empty())
        .unwrap_or(decoded.as_str());
    let title = format_stem(strip_extension(filename));

    if title.is_empty() {
        "Untitled Document".to_string()
    } else {
        title
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Strip the file extension from a name, returning the stem.
fn strip_extension(name: &str) -> &str {
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

/// Replace underscores and hyphens with spaces, then title-case each word.
fn format_stem(stem: &str) -> String {
    stem.replace(['_', '-'], " ")
        .split_whitespace()
        .map(title_case_word)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Capitalise the first Unicode scalar of `word`; leave the rest unchanged.
fn title_case_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Decode percent-encoded sequences in `s` (e.g. `%20` → `' '`).
///
/// Only ASCII bytes are decoded; non-ASCII multi-byte sequences are left as-is.
/// Invalid or incomplete sequences are passed through unchanged.
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16)
                && byte.is_ascii()
            {
                result.push(byte as char);
                i += 3;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap_or('\0');
        result.push(ch);
        i += ch.len_utf8();
    }
    result
}
