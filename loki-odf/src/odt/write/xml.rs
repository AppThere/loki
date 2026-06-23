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

/// The `style:page-layout` name for the section at `idx` (`PL1`, `PL2`, …).
#[must_use]
pub(super) fn page_layout_name(idx: usize) -> String {
    format!("PL{}", idx + 1)
}

/// Appends ` name="value"` to `out`, escaping the value.
pub(super) fn attr(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    out.push_str(&escape(value));
    out.push('"');
}
