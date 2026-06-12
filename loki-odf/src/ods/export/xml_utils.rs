// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! XML escaping and ODS formula conversion utilities.

pub(super) fn escape_xml(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(c),
        }
    }
    result
}

pub(super) fn to_ods_formula(formula: &str) -> String {
    let s = formula.trim();
    let s = s.strip_prefix('=').unwrap_or(s);

    let mut result = String::new();
    result.push_str("of:=");

    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let mut has_dollar1 = false;
        if i < chars.len() && chars[i] == '$' {
            has_dollar1 = true;
            i += 1;
        }

        let mut letters_len = 0;
        while i < chars.len() && chars[i].is_ascii_alphabetic() {
            i += 1;
            letters_len += 1;
        }

        let mut has_dollar2 = false;
        if letters_len > 0 && i < chars.len() && chars[i] == '$' {
            has_dollar2 = true;
            i += 1;
        }

        let mut digits_len = 0;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
            digits_len += 1;
        }

        if letters_len > 0 && digits_len > 0 && letters_len <= 3 {
            let next_ok = i == chars.len() || !chars[i].is_ascii_alphanumeric();
            let prev_ok = start == 0 || !chars[start - 1].is_ascii_alphanumeric();

            if next_ok && prev_ok {
                let mut ref_str = String::new();
                if has_dollar1 {
                    ref_str.push('$');
                }
                for idx in (start + if has_dollar1 { 1 } else { 0 })
                    ..(i - digits_len - if has_dollar2 { 1 } else { 0 })
                {
                    ref_str.push(chars[idx]);
                }
                if has_dollar2 {
                    ref_str.push('$');
                }
                for idx in (i - digits_len)..i {
                    ref_str.push(chars[idx]);
                }
                result.push_str(&format!("[.{}]", ref_str));
                continue;
            }
        }

        i = start;
        result.push(chars[i]);
        i += 1;
    }
    result.replace("]:[", ":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_ods_formula() {
        assert_eq!(to_ods_formula("A1+B2"), "of:=[.A1]+[.B2]");
        assert_eq!(to_ods_formula("SUM(A1:B10)"), "of:=SUM([.A1:.B10])");
        assert_eq!(to_ods_formula("=SUM(A1:B10)"), "of:=SUM([.A1:.B10])");
        assert_eq!(
            to_ods_formula("A1:C5+SUM($D$2:D$8)"),
            "of:=[.A1:.C5]+SUM([.$D$2:.D$8])"
        );
    }
}
