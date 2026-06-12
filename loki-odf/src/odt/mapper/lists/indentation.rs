// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Mapping of ODF list-level indentation to format-neutral `(indent_start, hanging_indent)`.

use loki_primitives::units::Points;

use crate::version::OdfVersion;
use crate::xml_util::parse_length;

/// Map positioning attributes to `(indent_start, hanging_indent)`.
///
/// Uses the ODF 1.2+ label-alignment model when the version supports it
/// and `label_followed_by` is `Some`; otherwise falls back to the legacy
/// ODF 1.1 `text:space-before` / `text:min-label-width` model.
pub(super) fn map_indentation(
    level: &crate::odt::model::list_styles::OdfListLevel,
    version: OdfVersion,
) -> (Points, Points) {
    let zero = Points::new(0.0);

    if version.supports_label_alignment() && level.label_followed_by.is_some() {
        // ODF 1.2+ label-alignment model
        let indent_start = level
            .margin_left
            .as_deref()
            .and_then(parse_length)
            .unwrap_or(zero);
        // text_indent is negative (hanging); store positive
        let hanging = level
            .text_indent
            .as_deref()
            .and_then(parse_length)
            .map_or(zero, |p| {
                if p.value() < 0.0 {
                    Points::new(-p.value())
                } else {
                    p
                }
            });
        (indent_start, hanging)
    } else {
        // ODF 1.1 legacy model: space_before + min_label_width = total indent
        let space_before = level
            .legacy_space_before
            .as_deref()
            .and_then(parse_length)
            .unwrap_or(zero);
        let label_width = level
            .legacy_min_label_width
            .as_deref()
            .and_then(parse_length)
            .unwrap_or(zero);
        let indent_start = Points::new(space_before.value() + label_width.value());
        let hanging = label_width;
        (indent_start, hanging)
    }
}
