// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Field-instruction parsing shared by the complex-field state machine and the
//! `w:fldSimple` mapper. ECMA-376 §17.16.

use loki_doc_model::content::field::types::{CrossRefFormat, FieldKind};

use crate::docx::model::paragraph::{DocxRun, DocxRunChild};

/// Parses an OOXML field instruction string into a [`FieldKind`].
///
/// The first word of the instruction (case-insensitive) identifies the field
/// type. Unknown types are stored as [`FieldKind::Raw`] for round-trip
/// fidelity. ADR-0005.
pub(super) fn parse_field_instruction(instruction: &str) -> FieldKind {
    let trimmed = instruction.trim();
    let first_word = trimmed.split_whitespace().next().unwrap_or("");

    match first_word.to_ascii_uppercase().as_str() {
        "PAGE" => FieldKind::PageNumber,
        "NUMPAGES" => FieldKind::PageCount,
        "DATE" => FieldKind::Date {
            format: extract_switch(trimmed, "@"),
        },
        "TIME" => FieldKind::Time {
            format: extract_switch(trimmed, "@"),
        },
        "TITLE" => FieldKind::Title,
        "AUTHOR" => FieldKind::Author,
        "SUBJECT" => FieldKind::Subject,
        "FILENAME" => FieldKind::FileName,
        "NUMWORDS" => FieldKind::WordCount,
        "REF" => {
            let target = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
            FieldKind::CrossReference {
                target,
                format: CrossRefFormat::Number,
            }
        }
        "PAGEREF" => {
            let target = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
            FieldKind::CrossReference {
                target,
                format: CrossRefFormat::Page,
            }
        }
        _ => FieldKind::Raw {
            instruction: trimmed.to_string(),
        },
    }
}

/// Extracts the value following a backslash-switch (e.g. `\@`) from a field
/// instruction string.
///
/// Returns the content of the first quoted string after `\{sw}`, or `None`
/// if the switch is not present.
fn extract_switch(instruction: &str, sw: &str) -> Option<String> {
    let needle = format!("\\{sw}");
    let pos = instruction.find(&needle)?;
    let rest = instruction[pos + needle.len()..].trim_start();
    if let Some(inner) = rest.strip_prefix('"') {
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        None
    }
}

/// Concatenates the visible text of a `w:fldSimple`'s cached-result runs.
pub(super) fn fld_simple_text(runs: &[DocxRun]) -> String {
    let mut out = String::new();
    for run in runs {
        for child in &run.children {
            if let DocxRunChild::Text { text, .. } = child {
                out.push_str(text);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_date_field_with_format_switch() {
        let kind = parse_field_instruction(r#" DATE \@ "MMMM d, yyyy" "#);
        assert!(matches!(kind, FieldKind::Date { format: Some(ref s) } if s == "MMMM d, yyyy"));
    }

    #[test]
    fn parse_ref_field() {
        let kind = parse_field_instruction(" REF _MyBookmark ");
        assert!(
            matches!(kind, FieldKind::CrossReference { target, format: CrossRefFormat::Number } if target == "_MyBookmark")
        );
    }

    #[test]
    fn parse_unknown_field_is_raw() {
        let kind = parse_field_instruction(" HYPERLINK \"https://example.com\" ");
        assert!(
            matches!(kind, FieldKind::Raw { instruction } if instruction.contains("HYPERLINK"))
        );
    }
}
