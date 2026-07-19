// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX run-property mapper: `DocxRPr` (`w:rPr`) → `CharProps`. Split out of
//! `props.rs` (Phase 7.1); re-exported from `props` so `super::props::map_rpr`
//! stays the stable path for the other mapper submodules.

use loki_doc_model::meta::LanguageTag;
use loki_doc_model::style::props::char_props::{
    CharProps, HighlightColor, StrikethroughStyle, VerticalAlign,
};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use crate::docx::model::paragraph::{DocxMarkRevision, DocxRPr};
use crate::xml_util::{hex_color, resolve_shading};

use super::{map_highlight, map_underline};

/// Maps a [`DocxRPr`] (OOXML run properties) to [`CharProps`].
///
/// Font sizes are in half-points (`w:sz`); letter spacing in twips (`w:spacing`).
/// Both are converted to points. Toggle properties map directly as `Option<bool>`.
pub(crate) fn map_rpr(rpr: &DocxRPr) -> CharProps {
    // Double strikethrough takes precedence over single.
    let strikethrough = match (rpr.dstrike, rpr.strike) {
        (Some(true), _) => Some(StrikethroughStyle::Double),
        (_, Some(true)) => Some(StrikethroughStyle::Single),
        _ => None,
    };

    // w:sz and w:szCs are in half-points.
    let font_size = rpr.sz.map(|hp| Points::new(f64::from(hp) / 2.0));
    let font_size_complex = rpr.sz_cs.map(|hp| Points::new(f64::from(hp) / 2.0));

    let (font_name, font_name_complex, font_name_east_asian) = if let Some(ref fonts) = rpr.fonts {
        (
            fonts.ascii.clone().or_else(|| fonts.h_ansi.clone()),
            fonts.cs.clone(),
            fonts.east_asia.clone(),
        )
    } else {
        (None, None, None)
    };

    // w:spacing is in twips.
    let letter_spacing = rpr.spacing.map(|sp| Points::new(f64::from(sp) / 20.0));

    // w:w is a percentage integer (100 = normal).
    #[allow(clippy::cast_precision_loss)]
    // Precision loss acceptable: values represent document measurements
    let scale = rpr.scale.map(|s| s as f32 / 100.0);

    // Run background from `w:shd`, honouring the pattern (`@w:val`): `pctN`
    // blends `@w:color` over `@w:fill`; `solid`/`clear` map as expected.
    let background_color = resolve_shading(
        rpr.shd_fill.as_deref(),
        rpr.shd_val.as_deref(),
        rpr.shd_color.as_deref(),
    )
    .map(DocumentColor::Rgb);

    CharProps {
        bold: rpr.bold,
        italic: rpr.italic,
        small_caps: rpr.small_caps,
        all_caps: rpr.all_caps,
        shadow: rpr.shadow,
        strikethrough,
        underline: rpr.underline.as_deref().and_then(map_underline),
        color: rpr
            .color
            .as_deref()
            .and_then(hex_color)
            .map(DocumentColor::Rgb),
        background_color,
        highlight_color: rpr
            .highlight
            .as_deref()
            .map(map_highlight)
            .filter(|h| *h != HighlightColor::None),
        font_size,
        font_size_complex,
        font_name,
        font_name_complex,
        font_name_east_asian,
        // w:kern threshold in half-points: 0 = off, >0 = enabled.
        kerning: rpr.kern.map(|k| k > 0),
        letter_spacing,
        scale,
        language: rpr.lang.as_deref().map(LanguageTag::new),
        language_complex: rpr.lang_complex.as_deref().map(LanguageTag::new),
        language_east_asian: rpr.lang_east_asian.as_deref().map(LanguageTag::new),
        vertical_align: rpr.vert_align.as_deref().and_then(|v| match v {
            "superscript" => Some(VerticalAlign::Superscript),
            "subscript" => Some(VerticalAlign::Subscript),
            _ => None,
        }),
        baseline_shift: rpr.position.map(|hp| Points::new(f64::from(hp) / 2.0)),
        outline: rpr.outline,
        emboss: rpr.emboss,
        imprint: rpr.imprint,
        // `w:bdr` character border, dropping an explicit none/nil edge.
        character_border: rpr
            .bdr
            .as_ref()
            .map(crate::docx::mapper::props::map_border_edge)
            .filter(|b| b.style != loki_doc_model::style::props::border::BorderStyle::None),
        // A paragraph mark's w:ins/w:del (tracked ¶ deletion) → CharProps.revision.
        revision: rpr.mark_rev.as_ref().map(DocxMarkRevision::to_mark),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::model::paragraph::DocxBorderEdge;
    use loki_doc_model::style::props::border::BorderStyle;

    #[test]
    fn maps_character_border() {
        let rpr = DocxRPr {
            bdr: Some(DocxBorderEdge {
                val: "single".into(),
                sz: Some(4),
                color: Some("C00000".into()),
                space: Some(0),
            }),
            ..Default::default()
        };
        let b = map_rpr(&rpr).character_border.expect("bdr mapped");
        assert_eq!(b.style, BorderStyle::Solid);
    }

    #[test]
    fn none_valued_bdr_is_dropped() {
        let rpr = DocxRPr {
            bdr: Some(DocxBorderEdge {
                val: "none".into(),
                sz: None,
                color: None,
                space: None,
            }),
            ..Default::default()
        };
        assert!(map_rpr(&rpr).character_border.is_none());
    }

    #[test]
    fn no_bdr_maps_to_none() {
        assert!(map_rpr(&DocxRPr::default()).character_border.is_none());
    }
    #[test]
    fn maps_emboss_and_imprint() {
        let rpr = DocxRPr {
            emboss: Some(true),
            imprint: Some(true),
            ..Default::default()
        };
        let cp = map_rpr(&rpr);
        assert_eq!(cp.emboss, Some(true));
        assert_eq!(cp.imprint, Some(true));
    }
}
