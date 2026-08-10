// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Fountain title page: leading `Key: value` pairs, ended by the first blank
//! line. `Title` maps to `TitlePageTitle`; every other value line maps to
//! `TitlePageLine` — the same two styles the bundled screenplay template's
//! title page uses, so a Fountain title page and a template title page are
//! the same thing to layout and export.

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::style::catalog::StyleId;

use crate::inline::parse_inlines;

/// A title page exists only if the first non-empty line is a `Key: value`
/// pair (per the spec). Returns the emitted title blocks and the remaining
/// body source.
pub(crate) fn split_title_page(text: &str) -> (Vec<Block>, &str) {
    let trimmed_start = text.trim_start_matches(['\n', '\r']);
    let Some(first_line) = trimmed_start.lines().next() else {
        return (Vec::new(), text);
    };
    if !is_key_line(first_line) {
        return (Vec::new(), text);
    }

    // The title page runs to the first blank line.
    let end = trimmed_start
        .find("\n\n")
        .or_else(|| trimmed_start.find("\r\n\r\n"))
        .unwrap_or(trimmed_start.len());
    let (page, body) = trimmed_start.split_at(end);

    let mut blocks = Vec::new();
    let mut current_key: Option<String> = None;
    for line in page.lines() {
        if let Some((key, value)) = split_key(line) {
            current_key = Some(key.to_ascii_lowercase());
            if !value.is_empty() {
                push_line(&mut blocks, current_key.as_deref(), value);
            }
        } else {
            // An indented continuation line belongs to the current key.
            let value = line.trim();
            if !value.is_empty() {
                push_line(&mut blocks, current_key.as_deref(), value);
            }
        }
    }
    (blocks, body)
}

/// `Key: value` — the key is letters/spaces before the first colon.
fn is_key_line(line: &str) -> bool {
    split_key(line).is_some()
}

fn split_key(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == ' ' || c == '_')
    {
        return None;
    }
    // A continuation line is indented; a key line starts at column zero.
    if line.starts_with([' ', '\t']) {
        return None;
    }
    Some((key, value.trim()))
}

/// One title-page value line as a styled paragraph.
fn push_line(blocks: &mut Vec<Block>, key: Option<&str>, value: &str) {
    let style = if key == Some("title") {
        "TitlePageTitle"
    } else {
        "TitlePageLine"
    };
    blocks.push(Block::StyledPara(StyledParagraph {
        style_id: Some(StyleId::new(style)),
        direct_para_props: None,
        direct_char_props: None,
        inlines: parse_inlines(value),
        attr: Default::default(),
    }));
}

#[cfg(test)]
mod tests {
    use super::split_title_page;
    use loki_doc_model::content::block::Block;

    fn style_of(block: &Block) -> &str {
        match block {
            Block::StyledPara(p) => p.style_id.as_ref().map_or("", |s| s.as_str()),
            _ => "",
        }
    }

    #[test]
    fn title_page_splits_and_styles() {
        let src = "Title: BIG FISH\nCredit: written by\nAuthor: A. Person\n\nFADE IN:";
        let (blocks, body) = split_title_page(src);
        assert_eq!(blocks.len(), 3);
        assert_eq!(style_of(&blocks[0]), "TitlePageTitle");
        assert_eq!(style_of(&blocks[1]), "TitlePageLine");
        assert!(body.contains("FADE IN:"));
    }

    #[test]
    fn no_title_page_when_first_line_is_not_a_key() {
        let src = "INT. HOUSE - DAY\n\nAction.";
        let (blocks, body) = split_title_page(src);
        assert!(blocks.is_empty());
        assert_eq!(body, src);
    }

    #[test]
    fn indented_continuations_join_their_key() {
        let src = "Contact:\n    555-0100\n    someone@example.com\n\nBody";
        let (blocks, _) = split_title_page(src);
        assert_eq!(blocks.len(), 2, "two continuation lines");
        assert!(blocks.iter().all(|b| style_of(b) == "TitlePageLine"));
    }
}
