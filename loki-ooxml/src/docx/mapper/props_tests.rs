// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the DOCX props mapper (`super`). Extracted from props.rs (Phase 7.1 inline-test extraction).

// Tests assert exact-representable point values converted from integer twips.
#![allow(clippy::float_cmp)]

use super::*;
use crate::docx::model::paragraph::{DocxInd, DocxNumPr, DocxRPr, DocxSpacing};
use loki_doc_model::style::props::char_props::StrikethroughStyle;

fn ppr_with_jc(jc: &str) -> DocxPPr {
    DocxPPr {
        jc: Some(jc.into()),
        ..Default::default()
    }
}

#[test]
fn frame_pr_drop_maps_to_drop_cap() {
    let ppr = DocxPPr {
        frame_pr: Some(DocxFramePr {
            drop_cap: Some("drop".into()),
            lines: Some(3),
            h_space: Some(40), // twips → 2pt
        }),
        ..Default::default()
    };
    let dc = map_ppr(&ppr).drop_cap.expect("drop cap present");
    assert_eq!(dc.lines, 3);
    assert!(!dc.margin);
    assert_eq!(dc.distance.value(), 2.0);
    assert_eq!(dc.length, DropCapLength::Chars(1));
}

#[test]
fn frame_pr_margin_is_in_margin() {
    let ppr = DocxPPr {
        frame_pr: Some(DocxFramePr {
            drop_cap: Some("margin".into()),
            lines: Some(2),
            h_space: None,
        }),
        ..Default::default()
    };
    assert!(map_ppr(&ppr).drop_cap.expect("drop cap").margin);
}

#[test]
fn frame_pr_none_is_not_a_drop_cap() {
    let ppr = DocxPPr {
        frame_pr: Some(DocxFramePr {
            drop_cap: Some("none".into()),
            lines: None,
            h_space: None,
        }),
        ..Default::default()
    };
    assert!(map_ppr(&ppr).drop_cap.is_none());
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
    assert_eq!(
        props
            .list_id
            .as_ref()
            .map(loki_doc_model::style::ListId::as_str),
        Some("3")
    );
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
fn position_maps_to_baseline_shift_in_points() {
    // w:position is in half-points; +12 = 6 pt up, -12 = 6 pt down.
    let up = map_rpr(&DocxRPr {
        position: Some(12),
        ..Default::default()
    });
    assert_eq!(up.baseline_shift.unwrap().value(), 6.0);
    let down = map_rpr(&DocxRPr {
        position: Some(-12),
        ..Default::default()
    });
    assert_eq!(down.baseline_shift.unwrap().value(), -6.0);
    assert!(map_rpr(&DocxRPr::default()).baseline_shift.is_none());
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
