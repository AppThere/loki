// SPDX-License-Identifier: Apache-2.0

//! The resolved-vs-overridden inspector model for **character styles**
//! (Spec 05 M6 — the character family).
//!
//! Mirrors [`super::style_inspector::paragraph_inspector_rows`] but resolves a
//! standalone [`CharacterStyle`]'s own inheritance chain via
//! `loki_doc_model`'s `resolve_char_chain` (the standalone-character resolver
//! the collapsing `resolve_char` never provided — audit SM-2). It emits the same
//! [`InspectorRow`] shape the paragraph family uses, so the provenance view and
//! reset/jump affordances are shared across families — the "uniform inspector"
//! Spec 05 §9 requires. Character styles carry only character properties, so the
//! row set is font family / size / bold / italic.

use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::{CharacterStyle, Provenance, StyleCatalog, StyleId};

use super::style_inspector::{InspectorRow, RowProvenance, StyleProperty};

/// Builds the inspector rows for the character style `id`, in display order.
///
/// Every applicable character property appears with its resolved value and
/// provenance (Local / Inherited-from / Default / FormatDefault). The `Default`
/// level comes from the catalog's `default_character_style` (the document's
/// `docDefaults` run defaults, ADR-0012 Decision 1) when set. Returns an empty
/// vector when `id` is not a character style in the catalog.
#[must_use]
pub fn character_inspector_rows(catalog: &StyleCatalog, id: &StyleId) -> Vec<InspectorRow> {
    if !catalog.character_styles.contains_key(id) {
        return Vec::new();
    }
    [
        build_row(
            catalog,
            id,
            StyleProperty::FontFamily,
            |s| s.char_props.font_name.clone(),
            fmt_string,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::FontSize,
            |s| s.char_props.font_size,
            fmt_points,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::Bold,
            |s| s.char_props.bold,
            fmt_bool,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::Italic,
            |s| s.char_props.italic,
            fmt_bool,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn build_row<T: Clone>(
    catalog: &StyleCatalog,
    id: &StyleId,
    property: StyleProperty,
    get: impl Fn(&CharacterStyle) -> Option<T>,
    fmt: impl Fn(&T) -> String,
) -> Option<InspectorRow> {
    let resolved = catalog.resolve_char_chain(id, get)?;
    Some(InspectorRow {
        property,
        value_display: resolved.value.as_ref().map(fmt),
        provenance: to_row_provenance(catalog, resolved.provenance),
    })
}

/// Maps model provenance to display-ready provenance, resolving an inherited
/// ancestor's display name from the **character** style map.
fn to_row_provenance(catalog: &StyleCatalog, p: Provenance) -> RowProvenance {
    match p {
        Provenance::Local => RowProvenance::Local,
        Provenance::Inherited(ancestor_id) => {
            let ancestor_display = catalog
                .character_styles
                .get(&ancestor_id)
                .and_then(|s| s.display_name.clone())
                .unwrap_or_else(|| ancestor_id.as_str().to_string());
            RowProvenance::Inherited {
                ancestor_id,
                ancestor_display,
            }
        }
        Provenance::Default => RowProvenance::Default,
        Provenance::FormatDefault => RowProvenance::FormatDefault,
    }
}

// ── Value formatters (character subset) ─────────────────────────────────────

// `&String` is required to satisfy the generic `fmt: Fn(&T)` bound (T = String).
#[allow(clippy::ptr_arg)]
fn fmt_string(s: &String) -> String {
    s.clone()
}

fn fmt_points(p: &Points) -> String {
    format!("{:.0} pt", p.value())
}

fn fmt_bool(b: &bool) -> String {
    if *b {
        "On".to_string()
    } else {
        "Off".to_string()
    }
}

#[cfg(test)]
#[path = "style_char_inspector_tests.rs"]
mod tests;
