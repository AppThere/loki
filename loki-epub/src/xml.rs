// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Minimal XML/XHTML text helpers shared by the EPUB serialisers.

/// Escapes the five predefined XML entities for use in element text content.
///
/// Escapes `&`, `<`, and `>`. Suitable for text nodes; for attribute values
/// use [`escape_attr`], which additionally escapes quotes.
#[must_use]
pub fn escape_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

/// Escapes a string for use inside a double-quoted XML attribute value.
#[must_use]
pub fn escape_attr(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_escaping() {
        assert_eq!(escape_text("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn attr_escaping() {
        assert_eq!(escape_attr("\"x'y\""), "&quot;x&#39;y&quot;");
    }
}
