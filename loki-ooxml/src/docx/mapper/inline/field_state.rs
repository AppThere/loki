// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OOXML complex-field state machine and field instruction parser.

use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};
use loki_doc_model::content::inline::Inline;

use crate::error::OoxmlWarning;

// ── Field state machine ────────────────────────────────────────────────────────

/// Assembly state for OOXML complex fields (`w:fldChar`/`w:instrText`).
///
/// `depth` tracks nested `begin` elements so that fields embedded inside
/// other field instructions are handled without panicking.
pub(super) enum FieldState {
    Normal,
    /// Accumulating field instruction text. `depth ≥ 1`.
    InCode {
        instruction: String,
        depth: usize,
    },
    /// After `w:fldChar @w:fldCharType="separate"`: accumulating snapshot text.
    InResult {
        instruction: String,
        snapshot: String,
        depth: usize,
    },
}

// ── Field state transitions ────────────────────────────────────────────────────

pub(super) fn process_fld_char(
    fld_char_type: &str,
    state: &mut FieldState,
    raw: &mut Vec<Inline>,
    warnings: &mut Vec<OoxmlWarning>,
) {
    match fld_char_type {
        "begin" => {
            let new_state = match &*state {
                FieldState::Normal => FieldState::InCode {
                    instruction: String::new(),
                    depth: 1,
                },
                FieldState::InCode { instruction, depth } => FieldState::InCode {
                    instruction: instruction.clone(),
                    depth: depth + 1,
                },
                FieldState::InResult {
                    instruction,
                    snapshot,
                    depth,
                } => FieldState::InResult {
                    instruction: instruction.clone(),
                    snapshot: snapshot.clone(),
                    depth: depth + 1,
                },
            };
            *state = new_state;
        }
        "separate" => {
            // Only transition at depth == 1; deeper separates belong to nested fields.
            if let FieldState::InCode { instruction, depth } = &*state
                && *depth == 1
            {
                let new_state = FieldState::InResult {
                    instruction: instruction.clone(),
                    snapshot: String::new(),
                    depth: 1,
                };
                *state = new_state;
            }
        }
        "end" => {
            match state {
                FieldState::InCode { depth, .. } | FieldState::InResult { depth, .. }
                    if *depth > 1 =>
                {
                    *depth -= 1;
                }
                FieldState::InCode { instruction, .. } => {
                    let kind = parse_field_instruction(instruction);
                    raw.push(Inline::Field(Field::new(kind)));
                    *state = FieldState::Normal;
                }
                FieldState::InResult {
                    instruction,
                    snapshot,
                    ..
                } => {
                    let kind = parse_field_instruction(instruction);
                    let cv = snapshot.trim().to_string();
                    let mut field = Field::new(kind);
                    if !cv.is_empty() {
                        field.current_value = Some(cv);
                    }
                    raw.push(Inline::Field(field));
                    *state = FieldState::Normal;
                }
                FieldState::Normal => {
                    // Malformed: `end` with no matching `begin`.
                    warnings.push(OoxmlWarning::UnrecognisedField {
                        instruction: "<malformed:end-without-begin>".to_string(),
                    });
                }
            }
        }
        _ => {} // Unknown fldCharType; ignore.
    }
}

// ── Field instruction parser ───────────────────────────────────────────────────

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
