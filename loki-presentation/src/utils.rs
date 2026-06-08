// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Application-level utility functions for `loki-presentation`.

use loki_file_access::FileAccessToken;

// ── display_title_from_path ───────────────────────────────────────────────────

/// Derive a human-readable document title from a raw route `path` segment.
pub fn display_title_from_path(path: &str) -> String {
    if path.is_empty() {
        return "Untitled Document".to_string();
    }

    if let Ok(token) = FileAccessToken::deserialize(path) {
        let title = format_stem(strip_extension(token.display_name()));
        if !title.is_empty() {
            return title;
        }
    }

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

fn strip_extension(name: &str) -> &str {
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

fn format_stem(stem: &str) -> String {
    stem.replace(['_', '-'], " ")
        .split_whitespace()
        .map(title_case_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_case_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

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
