// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List-marker synthesis: bullet / numbering label formatting.
//!
//! Split from `para.rs` (300-line-ceiling backlog, Q-1) — self-contained
//! formatting helpers with no layout-state dependencies.

use loki_doc_model::style::list_style::{BulletChar, ListLevel, ListLevelKind, NumberingScheme};

// ── List marker synthesis ─────────────────────────────────────────────────────

/// Produce the display string for a list marker at `level` in `list_levels`.
///
/// Handles bullet characters, all six [`NumberingScheme`] variants, and
/// multi-level `%N`-style format strings (OOXML `w:lvlText`, ODF
/// `text:num-format`). Picture bullets fall back to `"•"`.
///
/// # Arguments
/// * `list_levels` – all level definitions for the list (from `ListStyle.levels`)
/// * `level`       – the zero-based level being rendered
/// * `counters`    – current per-level counter array (all 9 levels)
///
/// Returns an empty string for `ListLevelKind::None`.
pub fn format_list_marker(list_levels: &[ListLevel], level: u8, counters: &[u32; 9]) -> String {
    let Some(level_def) = list_levels.get(level as usize) else {
        return String::new();
    };
    match &level_def.kind {
        ListLevelKind::Bullet {
            char: BulletChar::Char(c),
            ..
        } => c.to_string(),
        ListLevelKind::Bullet {
            char: BulletChar::Image { .. },
            ..
        } => {
            // A picture bullet has no text glyph; the flow engine places the
            // image out-of-band (see `flow_para`). Emit no marker text so the
            // label box is not double-filled. Callers that only need a text
            // marker (e.g. plain-text export) fall back to `•` themselves.
            String::new()
        }
        ListLevelKind::Numbered { format, .. } => {
            format_numbered_label(list_levels, format, counters)
        }
        ListLevelKind::None => String::new(),
        // Non-exhaustive guard.
        _ => String::new(),
    }
}

/// Expand a `w:lvlText`-style format string, replacing `%N` tokens with
/// the counter at 0-based level N-1 formatted by that level's scheme.
fn format_numbered_label(list_levels: &[ListLevel], format: &str, counters: &[u32; 9]) -> String {
    let mut result = String::with_capacity(format.len() + 4);
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%'
            && let Some(&d) = chars.peek()
            && d.is_ascii_digit()
            && d != '0'
        {
            chars.next();
            let level_idx = (d as u8 - b'1') as usize; // 1-based → 0-based
            let counter = counters.get(level_idx).copied().unwrap_or(1);
            let scheme = list_levels
                .get(level_idx)
                .map(|l| match &l.kind {
                    ListLevelKind::Numbered { scheme, .. } => *scheme,
                    _ => NumberingScheme::Decimal,
                })
                .unwrap_or(NumberingScheme::Decimal);
            result.push_str(&format_counter(counter, scheme));
            continue;
        }
        result.push(c);
    }
    result
}

/// Format a single counter value according to its numbering scheme.
///
/// Shared by list-marker rendering and page-number fields (OOXML
/// `w:pgNumType @w:fmt`).
pub(crate) fn format_counter(n: u32, scheme: NumberingScheme) -> String {
    match scheme {
        NumberingScheme::Decimal => n.to_string(),
        NumberingScheme::LowerAlpha => alpha_label(n, false),
        NumberingScheme::UpperAlpha => alpha_label(n, true),
        NumberingScheme::LowerRoman => roman_numeral(n, false),
        NumberingScheme::UpperRoman => roman_numeral(n, true),
        NumberingScheme::Ordinal => format!("{}{}", n, ordinal_suffix(n)),
        NumberingScheme::None => String::new(),
        _ => n.to_string(), // non-exhaustive fallback
    }
}

/// Convert `n` to an alphabetic label: 1→a, 2→b, …, 26→z, 27→aa, 28→ab, …
fn alpha_label(mut n: u32, upper: bool) -> String {
    let mut buf = Vec::new();
    while n > 0 {
        n -= 1;
        let byte = b'a' + (n % 26) as u8;
        buf.push(if upper {
            byte.to_ascii_uppercase()
        } else {
            byte
        });
        n /= 26;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap_or_default()
}

/// Convert `n` to a Roman numeral string.
fn roman_numeral(n: u32, upper: bool) -> String {
    const TABLE: &[(u32, &str)] = &[
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut n = n;
    let mut s = String::new();
    for &(val, sym) in TABLE {
        while n >= val {
            s.push_str(sym);
            n -= val;
        }
    }
    if upper { s.to_uppercase() } else { s }
}

/// Return the English ordinal suffix for `n` (1st, 2nd, 3rd, …, 11th, …).
fn ordinal_suffix(n: u32) -> &'static str {
    match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    }
}
