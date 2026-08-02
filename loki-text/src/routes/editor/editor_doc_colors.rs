// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The colours this document already uses, for the picker's document group
//! (Spec 08 T5.2).
//!
//! # "Document colours" means the style catalog, and that is a choice
//!
//! There are two defensible readings. One is *every colour appearing anywhere in
//! the content* — which needs an exhaustive walk over 17 `Block` variants and the
//! `Inline` tree beneath them, where a missed variant under-reports **silently**:
//! the group looks populated, and the colour the reader was looking for is the
//! one that is absent.
//!
//! The other is *the colours this document's styles define*, which is a flat
//! iteration over `IndexMap`s that cannot miss anything. It is also the more
//! useful list: a colour that a style defines is one the document uses
//! deliberately, where a colour found in the content may be a single word
//! somebody pasted.
//!
//! This takes the second. The first is a superset and would be a better list *if*
//! it could be trusted; it needs a visitor the model does not have, and building
//! one by hand is the part that would be wrong without saying so.
//! TODO(doc-colours-content): widen to content colours once `loki-doc-model`
//! grows an exhaustive inline visitor.
//!
//! # Order is the catalog's, not sorted
//!
//! `IndexMap` preserves insertion order, which for an imported document is the
//! order the styles appear in the file — stable across opens of the same
//! document. Sorting by hue or frequency would reorder the group whenever the
//! document changed, and a swatch that moves between visits is one the reader
//! has to search for each time.

use std::collections::HashSet;

use loki_doc_model::Document;
use loki_doc_model::loki_primitives::color::DocumentColor;

/// The most colours the group will show.
///
/// Two rows of six. A document with fifty distinct style colours has a palette
/// rather than a set of choices, and past the second row the group stops being
/// something a reader scans and becomes something they search.
const MAX_DOC_COLORS: usize = 12;

/// Distinct `#RRGGBB` colours defined by this document's styles, in catalog
/// order.
///
/// Only colours that resolve to RGB are offered — which is what
/// [`DocumentColor::to_hex`] already decides. A CMYK or theme colour has no
/// honest hex to put in a swatch: theme colours in particular resolve only
/// against a `ThemeColor` that **no importer in this workspace builds**, so a
/// theme swatch today would show an approximation of a colour the document never
/// specified. Absent beats approximate.
#[must_use]
pub(super) fn document_colors(doc: &Document) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    // `DocumentColor::to_hex` is the one derivation, and it already returns
    // `None` for exactly the cases that have no honest swatch — CMYK, and a
    // theme reference that resolves only against a `ThemeColor` no importer
    // builds. Re-matching the variants here would be a second copy of that
    // judgement, and the copy is what drifts.
    let mut push = |color: Option<&DocumentColor>| {
        let Some(hex) = color.and_then(DocumentColor::to_hex) else {
            return;
        };
        if seen.insert(hex.clone()) {
            out.push(hex);
        }
    };

    for style in doc.styles.character_styles.values() {
        push(style.char_props.color.as_ref());
        push(style.char_props.background_color.as_ref());
    }
    for style in doc.styles.paragraph_styles.values() {
        push(style.char_props.color.as_ref());
        push(style.char_props.background_color.as_ref());
    }

    out.truncate(MAX_DOC_COLORS);
    out
}

#[cfg(test)]
#[path = "editor_doc_colors_tests.rs"]
mod tests;
