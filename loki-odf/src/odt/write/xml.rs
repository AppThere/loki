// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Small XML helpers shared by the ODT writers.

use loki_primitives::units::Points;

/// Escapes XML text content / attribute values.
#[must_use]
pub(super) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Formats a [`Points`] value as an ODF length string (e.g. `"36pt"`,
/// `"12.5pt"`), dropping a redundant `.00`.
#[must_use]
pub(super) fn pt(p: Points) -> String {
    let v = p.value();
    if (v - v.round()).abs() < 1e-6 {
        format!("{v:.0}pt")
    } else {
        format!("{v:.2}pt")
    }
}

/// The `style:master-page` name for the section at `idx`.
///
/// Section 0 uses the conventional `"Standard"` master (the importer's initial
/// master); later sections use `"MP{idx}"`. `content.xml` references these names
/// from the first paragraph of each section via `style:master-page-name`, and
/// `styles.xml` defines them — the two must agree, so both call this.
#[must_use]
pub(super) fn master_page_name(idx: usize) -> String {
    if idx == 0 {
        "Standard".to_string()
    } else {
        format!("MP{idx}")
    }
}

/// Coerces a page-style id into a valid XML `NCName` for use as a `style:name`.
///
/// ODF `style:name` must be an `NCName`: no whitespace or reserved punctuation, and
/// it may not start with a digit. Common ids (`PageStyle1`, `Standard`, a user's
/// `Body`) already qualify and pass through unchanged, so they round-trip
/// exactly; anything else has invalid characters replaced with `_` and a leading
/// digit prefixed with `_`. An empty id yields an empty string, so the caller
/// falls back to the positional name.
#[must_use]
pub(super) fn sanitize_ncname(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for c in id.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    // NCName cannot begin with a digit, '-' or '.'.
    if out
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || c == '-' || c == '.')
    {
        out.insert(0, '_');
    }
    out
}

/// Appends ` name="value"` to `out`, escaping the value.
pub(super) fn attr(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    out.push_str(&escape(value));
    out.push('"');
}
