// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The `Find`/`Replacement` object model (macro spec §6.1, phase 6).
//!
//! Exposes the canonical Word idiom over the neutral document body:
//!
//! ```text
//! Selection.Find.Text = "old"
//! Selection.Find.Replacement.Text = "new"   ' omit for a search-only Execute
//! found = Selection.Find.Execute            ' returns True if "old" was present
//! ```
//!
//! `Execute` **replaces every match** with `Replacement.Text` when a replacement
//! has been set (including the empty string, which deletes matches); otherwise
//! it only searches. The `Replace:=` argument is intentionally **not** parsed —
//! once the host seam drops argument names, its position is unreliable, so the
//! presence of `Replacement.Text` is the sole, predictable replace signal.
//! Matching honours `MatchCase` (ASCII case-fold when off) and `WholeWord`.
//! A search-only run gates `DocRead`; a replacing run additionally gates
//! `DocWrite` and records one [`super::DocEdit::SetText`].

use loki_basic::{RuntimeError, Value};

use super::{DocEdit, ExecutionHost, FIND, MacroBackend, REPLACEMENT};
use crate::capability::Capability;

/// The singleton search state behind the stateless `FIND`/`REPLACEMENT` handles.
#[derive(Debug, Default, Clone)]
pub(crate) struct FindState {
    /// `Find.Text` — the text to search for.
    pub(crate) text: String,
    /// `Replacement.Text` — `Some` (incl. `""`) means `Execute` replaces.
    pub(crate) replacement: Option<String>,
    /// `Find.MatchCase`.
    pub(crate) match_case: bool,
    /// `Find.WholeWord`.
    pub(crate) whole_word: bool,
}

/// `Selection`/`Range` members: `.Find` and `.Text`.
pub(super) fn selection_member<B: MacroBackend>(
    host: &mut ExecutionHost<B>,
    member: &str,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    match member {
        "find" => Ok(Value::Object(FIND)),
        "text" => {
            host.gate(Capability::DocRead)?;
            Ok(Value::Str(host.doc().text.clone()))
        }
        _ => Err(no_member()),
    }
}

/// `Find.*` members: `.Text`, `.Replacement`, `.MatchCase`, `.WholeWord`, and
/// `.Execute` (search / replace-all).
pub(super) fn find_member<B: MacroBackend>(
    host: &mut ExecutionHost<B>,
    member: &str,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    match member {
        "text" => Ok(Value::Str(host.doc().find.text.clone())),
        "replacement" => Ok(Value::Object(REPLACEMENT)),
        "matchcase" => Ok(Value::Bool(host.doc().find.match_case)),
        "wholeword" => Ok(Value::Bool(host.doc().find.whole_word)),
        "execute" => execute(host),
        _ => Err(no_member()),
    }
}

/// `Replacement.*` members: `.Text`.
pub(super) fn replacement_member<B: MacroBackend>(
    host: &mut ExecutionHost<B>,
    member: &str,
    _args: &[Value],
) -> Result<Value, RuntimeError> {
    match member {
        "text" => Ok(Value::Str(
            host.doc().find.replacement.clone().unwrap_or_default(),
        )),
        _ => Err(no_member()),
    }
}

/// Runs `Find.Execute`: searches the body for `Find.Text` and, if a replacement
/// is set, replaces every match. Returns `True` if the text was present.
fn execute<B: MacroBackend>(host: &mut ExecutionHost<B>) -> Result<Value, RuntimeError> {
    let needle = host.doc().find.text.clone();
    if needle.is_empty() {
        return Ok(Value::Bool(false));
    }
    host.gate(Capability::DocRead)?;
    let match_case = host.doc().find.match_case;
    let whole_word = host.doc().find.whole_word;
    let body: Vec<char> = host.doc().text.chars().collect();
    let pat: Vec<char> = needle.chars().collect();
    let matches = scan(&body, &pat, match_case, whole_word);
    let found = !matches.is_empty();

    if let Some(rep) = host.doc().find.replacement.clone()
        && found
    {
        host.gate(Capability::DocWrite)?;
        let new_body = rebuild(&body, &matches, pat.len(), &rep);
        host.doc_mut().text = new_body.clone();
        host.doc_mut().batch.edits.push(DocEdit::SetText(new_body));
    }
    Ok(Value::Bool(found))
}

/// Byte-independent, left-to-right, non-overlapping match scan over chars.
fn scan(hay: &[char], needle: &[char], match_case: bool, whole_word: bool) -> Vec<usize> {
    let mut out = Vec::new();
    if needle.is_empty() || needle.len() > hay.len() {
        return out;
    }
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        let hit = (0..needle.len()).all(|k| ci_eq(hay[i + k], needle[k], match_case));
        if hit && boundaries_ok(hay, i, needle.len(), whole_word) {
            out.push(i);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Whether a match at `start..start+len` sits on word boundaries (or `WholeWord`
/// is off).
fn boundaries_ok(hay: &[char], start: usize, len: usize, whole_word: bool) -> bool {
    if !whole_word {
        return true;
    }
    let before = start == 0 || !is_word_char(hay[start - 1]);
    let after = start + len == hay.len() || !is_word_char(hay[start + len]);
    before && after
}

/// Rebuilds the body with each match replaced by `rep`.
fn rebuild(hay: &[char], matches: &[usize], needle_len: usize, rep: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    let mut mi = 0;
    while i < hay.len() {
        if mi < matches.len() && matches[mi] == i {
            out.push_str(rep);
            i += needle_len;
            mi += 1;
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    out
}

/// Character equality honouring `MatchCase` (ASCII case-fold when off).
fn ci_eq(a: char, b: char, match_case: bool) -> bool {
    if match_case {
        a == b
    } else {
        a.eq_ignore_ascii_case(&b)
    }
}

/// Whether `c` is part of a word (for `WholeWord` boundary tests).
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The standard "object doesn't support this property or method" error (438).
fn no_member() -> RuntimeError {
    RuntimeError::new(438, "Object doesn't support this property or method")
}
