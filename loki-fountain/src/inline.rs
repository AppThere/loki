// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Fountain inline emphasis: `**bold**`, `*italic*`, `_underline_` — a
//! single-level scanner (Fountain permits nesting, but single-level covers
//! real scripts; unmatched markers stay literal, per the spec's "when in
//! doubt, print it" rule).

use loki_doc_model::content::inline::Inline;

/// Parses one line of text into inlines.
pub(crate) fn parse_inlines(text: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut literal = String::new();
    let mut rest = text;

    while !rest.is_empty() {
        let mut matched = false;
        for (open, close, wrap) in [
            ("**", "**", WrapKind::Strong),
            ("*", "*", WrapKind::Emph),
            ("_", "_", WrapKind::Underline),
        ] {
            if let Some(after) = rest.strip_prefix(open) {
                // A span needs a non-empty body and a closing marker.
                if let Some(end) = after.find(close)
                    && end > 0
                {
                    flush(&mut out, &mut literal);
                    let body = &after[..end];
                    let inner = vec![Inline::Str(body.to_string())];
                    out.push(match wrap {
                        WrapKind::Strong => Inline::Strong(inner),
                        WrapKind::Emph => Inline::Emph(inner),
                        WrapKind::Underline => Inline::Underline(inner),
                    });
                    rest = &after[end + close.len()..];
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }
        let mut chars = rest.chars();
        if let Some(c) = chars.next() {
            literal.push(c);
            rest = chars.as_str();
        }
    }
    flush(&mut out, &mut literal);
    out
}

enum WrapKind {
    Strong,
    Emph,
    Underline,
}

fn flush(out: &mut Vec<Inline>, literal: &mut String) {
    if !literal.is_empty() {
        out.push(Inline::Str(std::mem::take(literal)));
    }
}

#[cfg(test)]
mod tests {
    use super::parse_inlines;
    use loki_doc_model::content::inline::Inline;

    #[test]
    fn emphasis_spans_parse_and_unmatched_markers_stay_literal() {
        let inlines = parse_inlines("He *really* **means** _it_ 2*3");
        assert_eq!(
            inlines,
            vec![
                Inline::Str("He ".into()),
                Inline::Emph(vec![Inline::Str("really".into())]),
                Inline::Str(" ".into()),
                Inline::Strong(vec![Inline::Str("means".into())]),
                Inline::Str(" ".into()),
                Inline::Underline(vec![Inline::Str("it".into())]),
                Inline::Str(" 2*3".into()),
            ]
        );
    }
}
