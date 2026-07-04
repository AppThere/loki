// SPDX-License-Identifier: Apache-2.0

//! The resolved-vs-overridden inspector model (Spec 05 M2).
//!
//! The old style editor showed only a style's *locally set* fields, so everything
//! inherited was invisible (audit SM-3 — "local-only blindness"). This module
//! builds, from the `loki_doc_model` provenance resolver (Spec 05 M1), one
//! [`InspectorRow`] per *applicable* property — set locally, inherited from a
//! named ancestor, supplied by the document default, or an engine fallback — each
//! carrying its resolved value and where it comes from. The rendering layer maps
//! [`StyleProperty`] to a localized label and an edit control; this model is
//! pure and i18n-free, so it is unit-testable without a UI.
//!
//! Provenance names the source ancestor by its **display name** (falling back to
//! the stable id), while keeping the ancestor's [`StyleId`] for the
//! jump-to-ancestor affordance — display-name UI over id operations.

use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::props::para_props::{LineHeight, ParagraphAlignment, Spacing};
use loki_doc_model::style::{ParagraphStyle, Provenance, StyleCatalog, StyleId};

/// Which property an [`InspectorRow`] describes. The rendering layer maps this to
/// a localized label and the appropriate edit control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleProperty {
    /// Run-default font family (character aspect of a paragraph style).
    FontFamily,
    /// Run-default font size.
    FontSize,
    /// Run-default bold.
    Bold,
    /// Run-default italic.
    Italic,
    /// Paragraph alignment.
    Alignment,
    /// Start-edge indent.
    IndentStart,
    /// End-edge indent.
    IndentEnd,
    /// First-line indent.
    IndentFirstLine,
    /// Space before the paragraph.
    SpaceBefore,
    /// Space after the paragraph.
    SpaceAfter,
    /// Line height.
    LineHeight,
}

/// Where a row's value comes from, ready for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowProvenance {
    /// Set on the inspected style itself (an override; resettable).
    Local,
    /// Inherited from a named ancestor in the style's `parent` chain.
    Inherited {
        /// The ancestor's stable id (for the jump-to-ancestor affordance).
        ancestor_id: StyleId,
        /// The ancestor's display name (falls back to the id when unnamed).
        ancestor_display: String,
    },
    /// Supplied by the document default style (docDefaults fall-through).
    Default,
    /// Unset everywhere; the rendering engine supplies the value.
    FormatDefault,
}

impl RowProvenance {
    /// `true` when the value is set on the inspected style itself.
    #[must_use]
    pub fn is_local(&self) -> bool {
        matches!(self, RowProvenance::Local)
    }
}

/// One inspector row: a property, its resolved value (display string), and its
/// provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorRow {
    /// Which property this row edits.
    pub property: StyleProperty,
    /// The resolved value formatted for display; `None` when unset everywhere
    /// (a `FormatDefault` row — the engine decides).
    pub value_display: Option<String>,
    /// Where the value comes from.
    pub provenance: RowProvenance,
}

/// Builds the inspector rows for the paragraph style `id`, in display order.
///
/// Every applicable property appears, regardless of whether it is set locally —
/// the fix for the old panel's local-only blindness. Returns an empty vector when
/// `id` is not a paragraph style in the catalog.
#[must_use]
pub fn paragraph_inspector_rows(catalog: &StyleCatalog, id: &StyleId) -> Vec<InspectorRow> {
    if !catalog.paragraph_styles.contains_key(id) {
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
        build_row(
            catalog,
            id,
            StyleProperty::Alignment,
            |s| s.para_props.alignment,
            fmt_alignment,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::IndentStart,
            |s| s.para_props.indent_start,
            fmt_points,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::IndentEnd,
            |s| s.para_props.indent_end,
            fmt_points,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::IndentFirstLine,
            |s| s.para_props.indent_first_line,
            fmt_points,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::SpaceBefore,
            |s| s.para_props.space_before,
            fmt_spacing,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::SpaceAfter,
            |s| s.para_props.space_after,
            fmt_spacing,
        ),
        build_row(
            catalog,
            id,
            StyleProperty::LineHeight,
            |s| s.para_props.line_height,
            fmt_line_height,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Resolves one property with provenance and formats its value into a row.
fn build_row<T: Clone>(
    catalog: &StyleCatalog,
    id: &StyleId,
    property: StyleProperty,
    get: impl Fn(&ParagraphStyle) -> Option<T>,
    fmt: impl Fn(&T) -> String,
) -> Option<InspectorRow> {
    let resolved = catalog.resolve_para_chain(id, get)?;
    Some(InspectorRow {
        property,
        value_display: resolved.value.as_ref().map(fmt),
        provenance: to_row_provenance(catalog, resolved.provenance),
    })
}

/// Maps model [`Provenance`] to display-ready [`RowProvenance`], resolving an
/// inherited ancestor's display name (falling back to its id).
fn to_row_provenance(catalog: &StyleCatalog, p: Provenance) -> RowProvenance {
    match p {
        Provenance::Local => RowProvenance::Local,
        Provenance::Inherited(ancestor_id) => {
            let ancestor_display = catalog
                .paragraph_styles
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

/// Clears the local override of `property` on `style`, so the property falls
/// through to its inherited / default / engine value on the next resolution —
/// the "reset to inherited" action (Spec 05 §6). A no-op when the property was
/// not set locally.
pub fn clear_local_property(style: &mut ParagraphStyle, property: StyleProperty) {
    let pp = &mut style.para_props;
    let cp = &mut style.char_props;
    match property {
        StyleProperty::FontFamily => cp.font_name = None,
        StyleProperty::FontSize => cp.font_size = None,
        StyleProperty::Bold => cp.bold = None,
        StyleProperty::Italic => cp.italic = None,
        StyleProperty::Alignment => pp.alignment = None,
        StyleProperty::IndentStart => pp.indent_start = None,
        StyleProperty::IndentEnd => pp.indent_end = None,
        StyleProperty::IndentFirstLine => pp.indent_first_line = None,
        StyleProperty::SpaceBefore => pp.space_before = None,
        StyleProperty::SpaceAfter => pp.space_after = None,
        StyleProperty::LineHeight => pp.line_height = None,
    }
}

// ── Value formatters ──────────────────────────────────────────────────────────

// Must take `&String` (not `&str`) to satisfy the generic `fmt: Fn(&T) -> String`
// bound in `build_row`, where `T = String` for the font-family row.
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

fn fmt_alignment(a: &ParagraphAlignment) -> String {
    match a {
        ParagraphAlignment::Left => "Left",
        ParagraphAlignment::Right => "Right",
        ParagraphAlignment::Center => "Center",
        ParagraphAlignment::Justify => "Justify",
        ParagraphAlignment::Distribute => "Distribute",
        _ => "—",
    }
    .to_string()
}

fn fmt_spacing(s: &Spacing) -> String {
    match s {
        Spacing::Exact(pt) => format!("{:.0} pt", pt.value()),
        other => format!("{other:?}"),
    }
}

fn fmt_line_height(l: &LineHeight) -> String {
    match l {
        LineHeight::Multiple(ratio) => format!("{ratio:.2}×"),
        LineHeight::Exact(pt) => format!("{:.0} pt", pt.value()),
        LineHeight::AtLeast(pt) => format!("≥ {:.0} pt", pt.value()),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
#[path = "style_inspector_tests.rs"]
mod tests;
