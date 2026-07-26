// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Text cleaning for Parley, and the byte-index maps it produces.
//!
//! Extracted from `para.rs` (300-line ceiling, technique 3): a self-contained
//! cluster with its own reason to change — what Parley must not be shown, and
//! how editor byte offsets translate across the removal.
//!
//! The two maps returned here are one `usize` per source byte each and are a
//! known residency item: Spec 09 S9-2 shrinks them to `u32`, and to an identity
//! representation for the common case where nothing was removed.

use super::StyleSpan;

/// Strips characters Parley must not see (control chars and the BOM, keeping
/// `\t`/`\n`) and remaps the style spans onto the cleaned text.
///
/// Returns `(clean_text, clean_spans, orig_to_clean, clean_to_orig)` — the two
/// byte-index maps let editor hit-testing translate between the original and
/// cleaned coordinate spaces.
pub(super) fn clean_text_and_spans(
    text: &str,
    spans: &[StyleSpan],
) -> (String, Vec<StyleSpan>, Vec<usize>, Vec<usize>) {
    let mut clean_text = String::with_capacity(text.len());
    let mut orig_to_clean = vec![0; text.len() + 1];
    let mut clean_to_orig = Vec::with_capacity(text.len() + 1);

    let mut orig_idx = 0;
    let mut clean_idx = 0;

    for c in text.chars() {
        let c_len = c.len_utf8();
        // Drop `\t`: a tab is pure positioning (an inline box). Left in, it
        // shapes to a `.notdef` (fonts lacking a tab glyph, e.g. Arimo) whose
        // advance stacks on the box and overshoots the stop; byte maps anyway.
        let keep = c == '\n' || (!c.is_control() && c != '\u{feff}');
        if keep {
            for i in 0..c_len {
                orig_to_clean[orig_idx + i] = clean_idx + i;
                clean_to_orig.push(orig_idx + i);
            }
            clean_text.push(c);
            orig_idx += c_len;
            clean_idx += c_len;
        } else {
            for i in 0..c_len {
                orig_to_clean[orig_idx + i] = clean_idx;
            }
            orig_idx += c_len;
        }
    }
    orig_to_clean[orig_idx] = clean_idx;
    clean_to_orig.push(orig_idx);

    let clean_spans = spans
        .iter()
        .map(|span| {
            let mut clean_span = span.clone();
            let start = orig_to_clean
                .get(span.range.start)
                .copied()
                .unwrap_or(clean_idx);
            let end = orig_to_clean
                .get(span.range.end)
                .copied()
                .unwrap_or(clean_idx);
            clean_span.range = start..end;
            clean_span
        })
        .collect();

    (clean_text, clean_spans, orig_to_clean, clean_to_orig)
}
