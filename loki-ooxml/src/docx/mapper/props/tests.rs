// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#[cfg(test)]
mod tests {
    use loki_doc_model::style::props::char_props::StrikethroughStyle;
    use loki_doc_model::style::props::para_props::{LineHeight, ParagraphAlignment};

    use crate::docx::model::paragraph::{DocxInd, DocxNumPr, DocxPPr, DocxRPr, DocxSpacing};

    use super::super::char::map_rpr;
    use super::super::para::map_ppr;

    fn ppr_with_jc(jc: &str) -> DocxPPr {
        DocxPPr {
            jc: Some(jc.into()),
            ..Default::default()
        }
    }

    // ── map_ppr ──────────────────────────────────────────────────────────────

    #[test]
    fn twip_conversion_720() {
        let ppr = DocxPPr {
            ind: Some(DocxInd {
                left: Some(720),
                ..Default::default()
            }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        assert_eq!(props.indent_start.unwrap().value(), 36.0);
    }

    #[test]
    fn jc_both_maps_to_justify() {
        assert_eq!(
            map_ppr(&ppr_with_jc("both")).alignment,
            Some(ParagraphAlignment::Justify)
        );
    }

    #[test]
    fn jc_distribute_maps_to_justify() {
        assert_eq!(
            map_ppr(&ppr_with_jc("distribute")).alignment,
            Some(ParagraphAlignment::Justify)
        );
    }

    #[test]
    fn jc_center_maps_to_center() {
        assert_eq!(
            map_ppr(&ppr_with_jc("center")).alignment,
            Some(ParagraphAlignment::Center)
        );
    }

    #[test]
    fn line_auto_276_is_multiple_1_15() {
        let ppr = DocxPPr {
            spacing: Some(DocxSpacing {
                line: Some(276),
                line_rule: Some("auto".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        if let Some(LineHeight::Multiple(m)) = props.line_height {
            assert!((m - 1.15_f32).abs() < 0.001, "expected ~1.15, got {m}");
        } else {
            panic!("expected LineHeight::Multiple");
        }
    }

    #[test]
    fn line_exact_240_is_12pt() {
        let ppr = DocxPPr {
            spacing: Some(DocxSpacing {
                line: Some(240),
                line_rule: Some("exact".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        if let Some(LineHeight::Exact(pts)) = props.line_height {
            assert_eq!(pts.value(), 12.0);
        } else {
            panic!("expected LineHeight::Exact");
        }
    }

    #[test]
    fn outline_lvl_0_becomes_1() {
        let ppr = DocxPPr {
            outline_lvl: Some(0),
            ..Default::default()
        };
        assert_eq!(map_ppr(&ppr).outline_level, Some(1));
    }

    #[test]
    fn num_id_zero_is_none() {
        let ppr = DocxPPr {
            num_pr: Some(DocxNumPr { num_id: 0, ilvl: 0 }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        assert!(props.list_id.is_none());
    }

    #[test]
    fn num_id_3_ilvl_1() {
        let ppr = DocxPPr {
            num_pr: Some(DocxNumPr { num_id: 3, ilvl: 1 }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        assert_eq!(props.list_id.as_ref().map(|l| l.as_str()), Some("3"));
        assert_eq!(props.list_level, Some(1));
    }

    // ── map_rpr ──────────────────────────────────────────────────────────────

    #[test]
    fn half_point_24_is_12pt() {
        let rpr = DocxRPr {
            sz: Some(24),
            ..Default::default()
        };
        let props = map_rpr(&rpr);
        assert_eq!(props.font_size.unwrap().value(), 12.0);
    }

    #[test]
    fn bold_none_is_none() {
        let rpr = DocxRPr {
            bold: None,
            ..Default::default()
        };
        assert!(map_rpr(&rpr).bold.is_none());
    }

    #[test]
    fn bold_some_true() {
        let rpr = DocxRPr {
            bold: Some(true),
            ..Default::default()
        };
        assert_eq!(map_rpr(&rpr).bold, Some(true));
    }

    #[test]
    fn bold_some_false() {
        let rpr = DocxRPr {
            bold: Some(false),
            ..Default::default()
        };
        assert_eq!(map_rpr(&rpr).bold, Some(false));
    }

    #[test]
    fn dstrike_takes_precedence_over_strike() {
        let rpr = DocxRPr {
            dstrike: Some(true),
            strike: Some(true),
            ..Default::default()
        };
        assert_eq!(
            map_rpr(&rpr).strikethrough,
            Some(StrikethroughStyle::Double)
        );
    }
}
