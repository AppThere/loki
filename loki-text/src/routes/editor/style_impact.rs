// SPDX-License-Identifier: Apache-2.0

//! Impact preview for staged style edits (Spec 05 M4 / §7).
//!
//! Before Apply, the panel shows *which* dependent styles a change to the edited
//! style will also change — no surprise cascades. Given the set of properties
//! being changed on a style, [`affected_dependents`] unions the exact dependent
//! sets from `loki_doc_model`'s [`StyleCatalog::dependents_affected`] (which
//! excludes dependents shadowed by a closer override).

use std::collections::HashSet;

use loki_doc_model::style::{ParagraphStyle, StyleCatalog, StyleId};

use super::style_inspector::StyleProperty;

/// Whether `property` is set locally on `style` — the shadowing test the impact
/// walk uses (mirrors the property↔field mapping of `clear_local_property`).
fn property_is_set(style: &ParagraphStyle, property: StyleProperty) -> bool {
    let pp = &style.para_props;
    let cp = &style.char_props;
    match property {
        StyleProperty::FontFamily => cp.font_name.is_some(),
        StyleProperty::FontSize => cp.font_size.is_some(),
        StyleProperty::Bold => cp.bold.is_some(),
        StyleProperty::Italic => cp.italic.is_some(),
        StyleProperty::Alignment => pp.alignment.is_some(),
        StyleProperty::IndentStart => pp.indent_start.is_some(),
        StyleProperty::IndentEnd => pp.indent_end.is_some(),
        StyleProperty::IndentFirstLine => pp.indent_first_line.is_some(),
        StyleProperty::SpaceBefore => pp.space_before.is_some(),
        StyleProperty::SpaceAfter => pp.space_after.is_some(),
        StyleProperty::LineHeight => pp.line_height.is_some(),
    }
}

/// The dependent styles of `id` whose resolved value would change if the given
/// `changed` properties were applied — deduplicated, in first-seen order.
///
/// Empty when nothing is staged or no dependent inherits a changed property.
#[must_use]
pub fn affected_dependents(
    catalog: &StyleCatalog,
    id: &StyleId,
    changed: &[StyleProperty],
) -> Vec<StyleId> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for &property in changed {
        for dependent in catalog.dependents_affected(id, |s| property_is_set(s, property)) {
            if seen.insert(dependent.clone()) {
                out.push(dependent);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "style_impact_tests.rs"]
mod tests;
