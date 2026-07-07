// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Visual styling of tracked-change runs (Review tab, 4a.2 render slice).
//!
//! A run whose [`CharProps::revision`] is set is coloured by its author and
//! decorated by kind — an **insertion** is underlined, a **deletion** struck
//! through — matching Word / LibreOffice. Colour + decoration are derived at
//! layout time, so accepting/rejecting a change (which clears the mark) reverts
//! the run to its normal appearance with no stored styling to undo.

use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::RevisionKind;

use crate::color::LayoutColor;
use crate::para::{StrikethroughStyle, StyleSpan, UnderlineStyle};

/// Distinct author colours (RGB), cycled by author — chosen for contrast on a
/// white page. The first is used when a change carries no author.
const AUTHOR_COLORS: &[(f32, f32, f32)] = &[
    (0.78, 0.13, 0.13), // red
    (0.13, 0.35, 0.78), // blue
    (0.13, 0.55, 0.20), // green
    (0.55, 0.20, 0.62), // purple
    (0.80, 0.45, 0.05), // orange
    (0.10, 0.50, 0.55), // teal
];

/// The colour for `author`, chosen deterministically so the same author always
/// gets the same colour (a simple FNV-ish hash into the palette).
fn author_color(author: Option<&str>) -> LayoutColor {
    let idx = match author {
        Some(a) => {
            let h = a
                .bytes()
                .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
            h as usize % AUTHOR_COLORS.len()
        }
        None => 0,
    };
    let (r, g, b) = AUTHOR_COLORS[idx];
    LayoutColor::new(r, g, b, 1.0)
}

/// Applies tracked-change colouring + decoration to `span` from `props.revision`
/// (a no-op when the run carries no revision). An existing underline/strikethrough
/// is preserved; only an absent one is added for the change kind.
pub(crate) fn apply(span: &mut StyleSpan, props: &CharProps) {
    let Some(rev) = &props.revision else {
        return;
    };
    span.color = author_color(rev.author.as_deref());
    match rev.kind {
        RevisionKind::Insertion => {
            if span.underline.is_none() {
                span.underline = Some(UnderlineStyle::Single);
            }
        }
        RevisionKind::Deletion => {
            if span.strikethrough.is_none() {
                span.strikethrough = Some(StrikethroughStyle::Single);
            }
        }
    }
}

#[cfg(test)]
#[path = "revision_style_tests.rs"]
mod tests;
