// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XML escaping and coordinate conversion utilities.

pub(super) fn escape_xml(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

pub(super) fn coord_to_cell_ref(row: u32, col: u32) -> String {
    let mut col_str = String::new();
    let mut temp = col + 1;
    while temp > 0 {
        let modulo = (temp - 1) % 26;
        col_str.insert(0, (b'A' + modulo as u8) as char);
        temp = (temp - 1) / 26;
    }
    format!("{}{}", col_str, row + 1)
}
