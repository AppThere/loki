// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List export (usage audit §10 tier 2): the catalog's `text:list-style`
//! definitions, and the grouped nested `<text:list>` emitter for consecutive
//! `StyledPara.list_id` paragraphs — previously exported as bare styled
//! paragraphs, which destroyed the list on ODT export.
//!
//! Serialisation uses the legacy ODF 1.1 positioning model
//! (`text:space-before` + `text:min-label-width` directly on the level
//! element) because it is the model this crate's own reader maps back with
//! `indent_start = space_before + label_width`, `hanging = label_width` —
//! the exact inverse written here, so the geometry round-trips losslessly.

use loki_doc_model::content::block::Block;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{BulletChar, ListLevelKind, NumberingScheme};

use super::content::{Cx, write_block};
use super::xml::attr;

/// Deepest level the emitter nests to (the model defines 9 levels, 0-based).
const MAX_LEVEL: u8 = 8;

// ── text:list-style serialisation ─────────────────────────────────────────────

/// ODF `style:num-format` token for a numbering scheme (ODF 1.3 §20.396) —
/// the inverse of the mapper's `map_numbering_scheme`. Ordinal has no ODF
/// token and degrades to decimal; `None` keeps the level countable but blank.
fn num_format(scheme: NumberingScheme) -> &'static str {
    match scheme {
        NumberingScheme::LowerAlpha => "a",
        NumberingScheme::UpperAlpha => "A",
        NumberingScheme::LowerRoman => "i",
        NumberingScheme::UpperRoman => "I",
        NumberingScheme::None => "",
        _ => "1",
    }
}

/// Splits a `%N`-token format string into `(prefix, suffix)` — the text before
/// the first `%` and after the last `%N` token (`"(%1)"` → `("(", ")")`).
fn prefix_suffix(format: &str) -> (&str, &str) {
    let Some(first) = format.find('%') else {
        return ("", format);
    };
    let prefix = &format[..first];
    // The suffix starts after the last %N token's digits.
    let mut suffix_start = format.len();
    let bytes = format.as_bytes();
    let mut i = format.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b'%' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            suffix_start = j;
            break;
        }
    }
    (prefix, &format[suffix_start..])
}

/// Writes one list level element.
fn write_level(out: &mut String, lvl: &loki_doc_model::style::list_style::ListLevel) {
    let level_attr = (lvl.level + 1).to_string(); // ODF levels are 1-based
    let space_before = (lvl.indent_start.value() - lvl.hanging_indent.value()).max(0.0);
    let label_width = lvl.hanging_indent.value();

    if let ListLevelKind::Numbered {
        scheme,
        start_value,
        format,
        display_levels,
    } = &lvl.kind
    {
        let (prefix, suffix) = prefix_suffix(format);
        out.push_str("<text:list-level-style-number");
        attr(out, "text:level", &level_attr);
        attr(out, "style:num-format", num_format(*scheme));
        if !prefix.is_empty() {
            attr(out, "style:num-prefix", prefix);
        }
        if !suffix.is_empty() {
            attr(out, "style:num-suffix", suffix);
        }
        if *start_value != 1 {
            attr(out, "text:start-value", &start_value.to_string());
        }
        if *display_levels > 1 {
            attr(out, "text:display-levels", &display_levels.to_string());
        }
    } else {
        // Bullets; picture bullets and unknown future kinds degrade to their
        // glyph fallback, matching the DOCX writer and the importer's PUA
        // normalisation.
        let glyph = match &lvl.kind {
            ListLevelKind::Bullet {
                char: BulletChar::Char(c),
                ..
            } => *c,
            _ => '\u{2022}',
        };
        out.push_str("<text:list-level-style-bullet");
        attr(out, "text:level", &level_attr);
        attr(out, "text:bullet-char", &glyph.to_string());
    }
    out.push('>');
    out.push_str("<style:list-level-properties");
    attr(out, "text:space-before", &format!("{space_before}pt"));
    attr(out, "text:min-label-width", &format!("{label_width}pt"));
    out.push_str("/>");
    out.push_str(match &lvl.kind {
        ListLevelKind::Numbered { .. } => "</text:list-level-style-number>",
        _ => "</text:list-level-style-bullet>",
    });
}

/// Writes every catalog list style as a `<text:list-style>` into
/// `office:styles` (called from `styles_xml`).
pub(super) fn write_list_styles(out: &mut String, styles: &StyleCatalog) {
    for (id, style) in &styles.list_styles {
        out.push_str("<text:list-style");
        attr(out, "style:name", id.as_str());
        if let Some(name) = &style.display_name {
            attr(out, "style:display-name", name);
        }
        out.push('>');
        for lvl in &style.levels {
            write_level(out, lvl);
        }
        out.push_str("</text:list-style>");
    }
}

// ── Grouped nested <text:list> emission ──────────────────────────────────────

/// The `(list_id, level)` of a block, when it is a styled-para list item.
fn membership(block: &Block) -> Option<(&str, u8)> {
    let Block::StyledPara(sp) = block else {
        return None;
    };
    let props = sp.direct_para_props.as_ref()?;
    let id = props.list_id.as_ref()?.as_str();
    Some((id, props.list_level.unwrap_or(0).min(MAX_LEVEL)))
}

/// Writes `blocks`, wrapping each run of consecutive same-list styled
/// paragraphs in properly nested `<text:list>` markup. Every loop that walks
/// a block sequence (sections, table cells, note bodies, frames) should call
/// this instead of calling `write_block` per element — per-element calls
/// cannot see the run, so each item would become its own single-item list.
pub(super) fn write_blocks(out: &mut String, blocks: &[Block], cx: &mut Cx) {
    let mut i = 0;
    while i < blocks.len() {
        let Some((list_id, _)) = membership(&blocks[i]) else {
            write_block(out, &blocks[i], cx);
            i += 1;
            continue;
        };
        // Extend the run: consecutive items of the same list.
        let mut end = i + 1;
        while end < blocks.len() && membership(&blocks[end]).is_some_and(|(id, _)| id == list_id) {
            end += 1;
        }
        write_list_run(out, &blocks[i..end], list_id, cx);
        i = end;
    }
}

/// One item's paragraph, via the ordinary styled-para path (auto styles,
/// revision milestones — everything a non-list paragraph gets).
fn write_item_para(out: &mut String, block: &Block, cx: &mut Cx) {
    write_block(out, block, cx);
}

/// Emits one run as nested lists: a level-N item lives N lists deep, each
/// deeper list opening inside the previous item's `<text:list-item>` (ODF has
/// no level attribute on items — depth *is* the nesting). The style name goes
/// on the outermost `<text:list>` only; nested lists inherit it.
fn write_list_run(out: &mut String, items: &[Block], list_id: &str, cx: &mut Cx) {
    out.push_str("<text:list");
    attr(out, "style:name", list_id);
    out.push('>');
    out.push_str("<text:list-item>");
    let mut level: u8 = 0;

    for (idx, block) in items.iter().enumerate() {
        let (_, item_level) = membership(block).unwrap_or((list_id, 0));
        if idx == 0 || item_level > level {
            // Deeper (or the first item starting deep): each step opens a
            // sub-list inside the still-open list-item — no close/reopen.
            while level < item_level {
                out.push_str("<text:list><text:list-item>");
                level += 1;
            }
        } else {
            // Shallower or level: close down, then start a sibling item.
            while level > item_level {
                out.push_str("</text:list-item></text:list>");
                level -= 1;
            }
            out.push_str("</text:list-item><text:list-item>");
        }
        write_item_para(out, block, cx);
    }

    while level > 0 {
        out.push_str("</text:list-item></text:list>");
        level -= 1;
    }
    out.push_str("</text:list-item>");
    out.push_str("</text:list>");
}
