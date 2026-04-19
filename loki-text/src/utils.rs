// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Application-level utility functions for `loki-text`.

// ── display_title_from_path ───────────────────────────────────────────────────

/// Derive a human-readable document title from a raw route `path` segment.
///
/// The priority order is:
///
/// 1. **Document-model metadata** — when a `DocumentLayout` or doc-model
///    handle is available, its `title` field takes precedence.
///    See the `TODO(doc-model)` comment inside this function.
/// 2. **Filename stem** — the last path component with its extension stripped,
///    underscores and hyphens replaced with spaces, and each word
///    title-cased.
/// 3. **Fallback** — `"Untitled Document"` when the path is empty or yields
///    no usable characters after processing.
///
/// # URL decoding
///
/// The function percent-decodes the path before processing, so
/// `%20` and similar sequences are resolved before filename extraction.
///
/// # Examples
///
/// ```
/// use loki_text::utils::display_title_from_path;
///
/// assert_eq!(display_title_from_path("budget_draft.docx"), "Budget Draft");
/// assert_eq!(display_title_from_path(""), "Untitled Document");
/// ```
pub fn display_title_from_path(path: &str) -> String {
    // TODO(doc-model): when FileAccessToken::deserialize(path) and a loaded
    // DocumentLayout are available, return the non-empty metadata title first:
    //     if let Ok(token) = loki_file_access::FileAccessToken::deserialize(path) {
    //         if let Some(name) = token.display_name().filter(|n| !n.is_empty()) {
    //             return name;
    //         }
    //     }

    if path.is_empty() {
        return "Untitled Document".to_string();
    }

    // Percent-decode the path (e.g. %20 → space) before parsing.
    let decoded = percent_decode(path);

    // Extract the last component across both Unix and Windows separators.
    let filename = decoded
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or(decoded.as_str());

    // Strip the file extension.
    let stem = filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(filename);

    if stem.is_empty() {
        return "Untitled Document".to_string();
    }

    // Replace underscores and hyphens with spaces, then title-case each word.
    let title = stem
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(title_case_word)
        .collect::<Vec<_>>()
        .join(" ");

    if title.is_empty() {
        "Untitled Document".to_string()
    } else {
        title
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
/// Invalid or incomplete sequences are passed through unchanged.
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                // Only decode bytes that are valid ASCII; non-ASCII multi-byte
                // sequences would need a proper UTF-8 decoder — keep them raw.
                if byte.is_ascii() {
                    result.push(byte as char);
                    i += 3;
                    continue;
                }
            }
        }
        result.push(s[i..].chars().next().unwrap_or('\0'));
        i += s[i..].chars().next().map_or(1, |c| c.len_utf8());
    }
    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::display_title_from_path;

    #[test]
    fn normal_filename() {
        assert_eq!(display_title_from_path("budget_draft.docx"), "Budget Draft");
    }

    #[test]
    fn url_encoded_path() {
        assert_eq!(
            display_title_from_path("/home/user/meeting%20notes.odt"),
            "Meeting Notes",
        );
    }

    #[test]
    fn no_extension() {
        assert_eq!(display_title_from_path("README"), "README");
    }

    #[test]
    fn empty_string() {
        assert_eq!(display_title_from_path(""), "Untitled Document");
    }

    #[test]
    fn hyphen_separated() {
        assert_eq!(display_title_from_path("q1-report.docx"), "Q1 Report");
    }

    #[test]
    fn nested_path() {
        assert_eq!(
            display_title_from_path("/documents/work/invoice_draft.odt"),
            "Invoice Draft",
        );
    }
}
