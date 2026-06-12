// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Low-level XML helpers for ODS parsing.

pub(super) fn local_name<'a>(e: &'a quick_xml::events::BytesStart<'a>) -> &'a [u8] {
    let name = e.local_name().into_inner();
    if let Some(pos) = name.iter().position(|&b| b == b':') {
        &name[pos + 1..]
    } else {
        name
    }
}

pub(super) fn local_name_end<'a>(e: &'a quick_xml::events::BytesEnd<'a>) -> &'a [u8] {
    let name = e.local_name().into_inner();
    if let Some(pos) = name.iter().position(|&b| b == b':') {
        &name[pos + 1..]
    } else {
        name
    }
}

pub(super) fn clean_ods_formula(formula: &str) -> String {
    let s = formula.trim();
    let s = if let Some(stripped) = s.strip_prefix("of:=") {
        stripped
    } else if let Some(stripped) = s.strip_prefix("oooc:=") {
        stripped
    } else if let Some(stripped) = s.strip_prefix('=') {
        stripped
    } else {
        s
    };

    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                if chars.peek() == Some(&'.') {
                    chars.next();
                }
            }
            ']' => {}
            '.' => {
                if !result.ends_with(':') {
                    result.push(c);
                }
            }
            _ => {
                result.push(c);
            }
        }
    }
    format!("={result}")
}
