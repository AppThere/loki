// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cell property mapper: [`OdfCellProps`] → [`CellProps`].

use loki_doc_model::content::table::row::{CellProps, CellTextDirection, CellVerticalAlign};
use loki_primitives::color::DocumentColor;

use crate::odt::model::styles::OdfCellProps;
use crate::xml_util::parse_length;

use super::para::parse_odf_border;

/// Convert [`OdfCellProps`] to the format-neutral [`CellProps`].
///
/// All length strings are parsed via [`parse_length`]. Unparseable or absent
/// values silently map to `None`. ODF 1.3 §17.18.
///
/// NOTE: ODF cell properties are mapped to the same [`CellProps`] type as
/// OOXML. The layout engine applies them identically.
pub(crate) fn map_cell_props(cell_props: &OdfCellProps) -> CellProps {
    CellProps {
        padding_top: cell_props.padding_top.as_deref().and_then(parse_length),
        padding_bottom: cell_props.padding_bottom.as_deref().and_then(parse_length),
        padding_left: cell_props.padding_left.as_deref().and_then(parse_length),
        padding_right: cell_props.padding_right.as_deref().and_then(parse_length),
        vertical_align: cell_props
            .vertical_align
            .as_deref()
            .and_then(map_odf_vertical_align),
        text_direction: cell_props
            .writing_mode
            .as_deref()
            .and_then(map_odf_writing_mode),
        background_color: cell_props.background_color.as_deref().and_then(|c| {
            if c == "transparent" {
                None
            } else {
                DocumentColor::from_hex(c).ok()
            }
        }),
        border_top: cell_props.border_top.as_deref().and_then(parse_odf_border),
        border_bottom: cell_props
            .border_bottom
            .as_deref()
            .and_then(parse_odf_border),
        border_left: cell_props.border_left.as_deref().and_then(parse_odf_border),
        border_right: cell_props
            .border_right
            .as_deref()
            .and_then(parse_odf_border),
    }
}

/// Map an ODF `style:vertical-align` string to [`CellVerticalAlign`].
///
/// `"automatic"` falls through to the default `Top`.
pub(crate) fn map_odf_vertical_align(val: &str) -> Option<CellVerticalAlign> {
    match val {
        "top" | "automatic" => Some(CellVerticalAlign::Top),
        "middle" => Some(CellVerticalAlign::Middle),
        "bottom" => Some(CellVerticalAlign::Bottom),
        _ => None,
    }
}

/// Map an ODF `style:writing-mode` string to [`CellTextDirection`].
pub(crate) fn map_odf_writing_mode(val: &str) -> Option<CellTextDirection> {
    match val {
        "lr-tb" | "lr" => Some(CellTextDirection::LrTb),
        "tb-rl" | "tb" => Some(CellTextDirection::TbRl),
        "tb-lr" => Some(CellTextDirection::TbLr),
        "bt-lr" => Some(CellTextDirection::BtLr),
        _ => None,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::odt::model::styles::OdfCellProps;

    #[test]
    fn vertical_align_middle_maps_to_middle() {
        assert_eq!(
            map_odf_vertical_align("middle"),
            Some(CellVerticalAlign::Middle)
        );
    }

    #[test]
    fn vertical_align_top_maps_to_top() {
        assert_eq!(map_odf_vertical_align("top"), Some(CellVerticalAlign::Top));
    }

    #[test]
    fn vertical_align_automatic_maps_to_top() {
        assert_eq!(
            map_odf_vertical_align("automatic"),
            Some(CellVerticalAlign::Top)
        );
    }

    #[test]
    fn vertical_align_bottom_maps_to_bottom() {
        assert_eq!(
            map_odf_vertical_align("bottom"),
            Some(CellVerticalAlign::Bottom)
        );
    }

    #[test]
    fn vertical_align_unknown_returns_none() {
        assert_eq!(map_odf_vertical_align("baseline"), None);
    }

    #[test]
    fn writing_mode_tb_rl_maps_to_tbrl() {
        assert_eq!(map_odf_writing_mode("tb-rl"), Some(CellTextDirection::TbRl));
    }

    #[test]
    fn writing_mode_lr_tb_maps_to_lrtb() {
        assert_eq!(map_odf_writing_mode("lr-tb"), Some(CellTextDirection::LrTb));
    }

    #[test]
    fn writing_mode_lr_shorthand_maps_to_lrtb() {
        assert_eq!(map_odf_writing_mode("lr"), Some(CellTextDirection::LrTb));
    }

    #[test]
    fn fo_padding_shorthand_applies_to_all_edges() {
        let cell_props = OdfCellProps {
            padding_top: Some("0.2cm".into()),
            padding_bottom: Some("0.2cm".into()),
            padding_left: Some("0.2cm".into()),
            padding_right: Some("0.2cm".into()),
            ..Default::default()
        };
        let props = map_cell_props(&cell_props);
        // 0.2cm ≈ 5.669pt
        for (label, val) in [
            ("top", props.padding_top),
            ("bottom", props.padding_bottom),
            ("left", props.padding_left),
            ("right", props.padding_right),
        ] {
            let pts = val
                .unwrap_or_else(|| panic!("padding_{label} should be Some"))
                .value();
            assert!(
                (pts - 5.669).abs() < 0.1,
                "padding_{label} should be ~5.67pt, got {pts:.3}"
            );
        }
    }
}
