// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Loro bridge round-trip tests for L-severity gap fixes.
//!
//! Verifies that language, border, padding, page_break_before, orphan_control,
//! outline_level, tab_stops, and paragraph background_color all survive a
//! document_to_loro → loro_to_document cycle.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::meta::language::LanguageTag;
use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader, TabStop};
use loki_primitives::color::{DocumentColor, ThemeColorSlot};
use loki_primitives::units::Points;

fn round_trip(doc: &Document) -> Document {
    let loro = document_to_loro(doc).expect("document_to_loro must succeed");
    loro_to_document(&loro).expect("loro_to_document must succeed")
}

fn single_block_doc(block: Block) -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks.push(block);
    doc
}

fn styled_para_with_char(props: CharProps) -> Block {
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str("text".into())],
        attr: NodeAttr::default(),
    };
    Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::StyledRun(run)],
        attr: NodeAttr::default(),
    })
}

// ── bridge_language_roundtrip ─────────────────────────────────────────────────

/// `CharProps.language`, `language_complex`, and `language_east_asian` must all
/// survive a Loro CRDT round-trip via text marks.
#[test]
fn bridge_language_roundtrip() {
    let props = CharProps {
        language: Some(LanguageTag::new("en-GB".to_string())),
        language_complex: Some(LanguageTag::new("ar-SA".to_string())),
        language_east_asian: Some(LanguageTag::new("ja-JP".to_string())),
        ..Default::default()
    };

    let doc = single_block_doc(styled_para_with_char(props));
    let recovered = round_trip(&doc);

    let run_props = recovered.sections[0].blocks.iter().find_map(|b| {
        if let Block::StyledPara(p) = b {
            p.inlines.iter().find_map(|i| {
                if let Inline::StyledRun(sr) = i {
                    sr.direct_props.as_deref()
                } else {
                    None
                }
            })
        } else {
            None
        }
    });

    let rp = run_props.expect("StyledRun with direct_props must survive round-trip");

    assert_eq!(
        rp.language.as_ref().map(|t| t.as_str()),
        Some("en-GB"),
        "language must survive Loro round-trip"
    );
    assert_eq!(
        rp.language_complex.as_ref().map(|t| t.as_str()),
        Some("ar-SA"),
        "language_complex must survive Loro round-trip"
    );
    assert_eq!(
        rp.language_east_asian.as_ref().map(|t| t.as_str()),
        Some("ja-JP"),
        "language_east_asian must survive Loro round-trip"
    );
}

// ── bridge_border_roundtrip ───────────────────────────────────────────────────

/// `ParaProps.border_top` must survive a Loro CRDT round-trip.
#[test]
fn bridge_border_roundtrip() {
    let para_props = ParaProps {
        border_top: Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(1.0),
            color: Some(DocumentColor::from_hex("#FF0000").unwrap()),
            spacing: None,
        }),
        border_bottom: Some(Border {
            style: BorderStyle::Dashed,
            width: Points::new(0.5),
            color: None,
            spacing: Some(Points::new(2.0)),
        }),
        ..Default::default()
    };

    let block = Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(para_props)),
        direct_char_props: None,
        inlines: vec![Inline::Str("bordered".into())],
        attr: NodeAttr::default(),
    });

    let doc = single_block_doc(block);
    let recovered = round_trip(&doc);

    let para = recovered.sections[0]
        .blocks
        .iter()
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                Some(p)
            } else {
                None
            }
        })
        .expect("StyledPara must survive round-trip");

    let pp = para
        .direct_para_props
        .as_deref()
        .expect("direct_para_props must be Some");

    let top = pp
        .border_top
        .as_ref()
        .expect("border_top must survive round-trip");
    assert_eq!(top.style, BorderStyle::Solid);
    assert!(
        (top.width.value() - 1.0).abs() < 0.001,
        "border_top width must be 1.0, got {}",
        top.width.value()
    );
    assert_eq!(
        top.color.as_ref().and_then(|c| c.to_hex()).as_deref(),
        Some("#FF0000"),
        "border_top color must round-trip"
    );

    let bottom = pp
        .border_bottom
        .as_ref()
        .expect("border_bottom must survive round-trip");
    assert_eq!(bottom.style, BorderStyle::Dashed);
    assert!(
        (bottom.spacing.map_or(0.0, |s| s.value()) - 2.0).abs() < 0.001,
        "border_bottom spacing must be 2.0"
    );
}

/// Non-Rgb border colors (Theme / Cmyk) must survive the CRDT — formerly they
/// collapsed to `auto` because the v1 border string could not carry the
/// colon-delimited total color codec (loro-bridge tail, format migrated to v2).
#[test]
fn bridge_border_theme_and_cmyk_colors_roundtrip() {
    use loki_primitives::color::CmykColor;

    let para_props = ParaProps {
        border_top: Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(1.5),
            color: Some(DocumentColor::Theme {
                slot: ThemeColorSlot::Accent2,
                tint: 0.25,
            }),
            spacing: Some(Points::new(3.0)),
        }),
        border_bottom: Some(Border {
            style: BorderStyle::Double,
            width: Points::new(2.0),
            color: Some(DocumentColor::Cmyk(CmykColor::new(0.1, 0.2, 0.3, 0.4))),
            spacing: None,
        }),
        ..Default::default()
    };

    let block = Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(para_props)),
        direct_char_props: None,
        inlines: vec![Inline::Str("themed borders".into())],
        attr: NodeAttr::default(),
    });
    let recovered = round_trip(&single_block_doc(block));

    let pp = match &recovered.sections[0].blocks[0] {
        Block::StyledPara(p) => p.direct_para_props.as_deref().expect("para props"),
        other => panic!("expected StyledPara, got {other:?}"),
    };
    match pp.border_top.as_ref().and_then(|b| b.color.as_ref()) {
        Some(DocumentColor::Theme { slot, tint }) => {
            assert_eq!(*slot, ThemeColorSlot::Accent2);
            assert!((tint - 0.25).abs() < 1e-6);
        }
        other => panic!("theme border color must survive, got {other:?}"),
    }
    let top = pp.border_top.as_ref().unwrap();
    assert!((top.spacing.map_or(0.0, |s| s.value()) - 3.0).abs() < 0.001);
    match pp.border_bottom.as_ref().and_then(|b| b.color.as_ref()) {
        Some(DocumentColor::Cmyk(c)) => {
            assert!((c.cyan() - 0.1).abs() < 1e-4 && (c.key() - 0.4).abs() < 1e-4);
        }
        other => panic!("cmyk border color must survive, got {other:?}"),
    }
}

// ── bridge_tab_stops_roundtrip ────────────────────────────────────────────────

fn styled_para_with_para(props: ParaProps) -> Block {
    Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(props)),
        direct_char_props: None,
        inlines: vec![Inline::Str("text".into())],
        attr: NodeAttr::default(),
    })
}

fn recovered_para_props(doc: &Document) -> ParaProps {
    doc.sections[0]
        .blocks
        .iter()
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                p.direct_para_props.as_deref().cloned()
            } else {
                None
            }
        })
        .expect("StyledPara with direct_para_props must survive round-trip")
}

/// `ParaProps.tab_stops` must survive a Loro CRDT round-trip with position,
/// alignment, and leader intact (was written as an unreadable Debug string).
#[test]
fn bridge_tab_stops_roundtrip() {
    let para_props = ParaProps {
        tab_stops: Some(vec![
            TabStop {
                position: Points::new(36.0),
                alignment: TabAlignment::Left,
                leader: TabLeader::None,
            },
            TabStop {
                position: Points::new(144.5),
                alignment: TabAlignment::Decimal,
                leader: TabLeader::Dot,
            },
        ]),
        ..Default::default()
    };

    let doc = single_block_doc(styled_para_with_para(para_props));
    let pp = recovered_para_props(&round_trip(&doc));

    let stops = pp.tab_stops.as_ref().expect("tab_stops must survive");
    assert_eq!(stops.len(), 2, "both tab stops must survive");
    assert!((stops[0].position.value() - 36.0).abs() < 0.001);
    assert_eq!(stops[0].alignment, TabAlignment::Left);
    assert_eq!(stops[0].leader, TabLeader::None);
    assert!((stops[1].position.value() - 144.5).abs() < 0.001);
    assert_eq!(stops[1].alignment, TabAlignment::Decimal);
    assert_eq!(stops[1].leader, TabLeader::Dot);
}

// ── bridge_para_background_color_roundtrip ────────────────────────────────────

/// Paragraph `background_color` must survive a Loro CRDT round-trip (was
/// written as a Debug string the reader could not parse) — including non-Rgb
/// variants, which the codec must not collapse or drop.
#[test]
fn bridge_para_background_color_roundtrip() {
    for color in [
        DocumentColor::from_hex("#ABCDEF").unwrap(),
        DocumentColor::Cmyk(loki_primitives::color::CmykColor::new(0.1, 0.2, 0.3, 0.4)),
        DocumentColor::Theme {
            slot: ThemeColorSlot::Accent3,
            tint: 0.25,
        },
        DocumentColor::Transparent,
    ] {
        let para_props = ParaProps {
            background_color: Some(color.clone()),
            ..Default::default()
        };

        let doc = single_block_doc(styled_para_with_para(para_props));
        let pp = recovered_para_props(&round_trip(&doc));

        assert_eq!(
            pp.background_color,
            Some(color),
            "paragraph background_color must survive Loro round-trip"
        );
    }
}

// ── bridge_emboss_imprint_char_border_roundtrip ───────────────────────────────

/// `CharProps.emboss`, `imprint`, and `character_border` must survive a Loro
/// CRDT round-trip via text marks (previously read from OOXML/ODF but dropped by
/// the bridge — the last export-refinement round-trip gap for these fields).
#[test]
fn bridge_emboss_imprint_char_border_roundtrip() {
    let props = CharProps {
        emboss: Some(true),
        imprint: Some(true),
        character_border: Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(1.0),
            color: Some(DocumentColor::from_hex("#C00000").unwrap()),
            spacing: Some(Points::new(1.0)),
        }),
        ..Default::default()
    };

    let doc = single_block_doc(styled_para_with_char(props));
    let recovered = round_trip(&doc);

    let rp = recovered.sections[0]
        .blocks
        .iter()
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                p.inlines.iter().find_map(|i| match i {
                    Inline::StyledRun(sr) => sr.direct_props.as_deref(),
                    _ => None,
                })
            } else {
                None
            }
        })
        .expect("StyledRun with direct_props must survive round-trip");

    assert_eq!(rp.emboss, Some(true), "emboss must survive Loro round-trip");
    assert_eq!(
        rp.imprint,
        Some(true),
        "imprint must survive Loro round-trip"
    );
    let b = rp
        .character_border
        .as_ref()
        .expect("character_border must survive Loro round-trip");
    assert_eq!(b.style, BorderStyle::Solid);
    assert_eq!(b.width.value().round(), 1.0);
    assert_eq!(
        b.color,
        Some(DocumentColor::from_hex("#C00000").unwrap()),
        "border colour must survive"
    );
    assert_eq!(
        b.spacing.map(|p| p.value().round()),
        Some(1.0),
        "border spacing must survive"
    );
}

// ── bridge_para_fields_roundtrip ──────────────────────────────────────────────

/// `ParaProps.page_break_before`, `orphan_control`, `outline_level`, and
/// `padding_top/bottom/left/right` must survive a Loro CRDT round-trip.
#[test]
fn bridge_para_fields_roundtrip() {
    let para_props = ParaProps {
        page_break_before: Some(true),
        orphan_control: Some(2),
        outline_level: Some(3),
        padding_top: Some(Points::new(4.0)),
        padding_bottom: Some(Points::new(5.0)),
        padding_left: Some(Points::new(6.0)),
        padding_right: Some(Points::new(7.0)),
        ..Default::default()
    };

    let block = Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(para_props)),
        direct_char_props: None,
        inlines: vec![Inline::Str("padded".into())],
        attr: NodeAttr::default(),
    });

    let doc = single_block_doc(block);
    let recovered = round_trip(&doc);

    let para = recovered.sections[0]
        .blocks
        .iter()
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                Some(p)
            } else {
                None
            }
        })
        .expect("StyledPara must survive round-trip");

    let pp = para
        .direct_para_props
        .as_deref()
        .expect("direct_para_props must be Some");

    assert_eq!(
        pp.page_break_before,
        Some(true),
        "page_break_before must survive"
    );
    assert_eq!(pp.orphan_control, Some(2), "orphan_control must survive");
    assert_eq!(pp.outline_level, Some(3), "outline_level must survive");
    assert!(
        (pp.padding_top.map_or(0.0, |p| p.value()) - 4.0).abs() < 0.001,
        "padding_top must survive"
    );
    assert!(
        (pp.padding_bottom.map_or(0.0, |p| p.value()) - 5.0).abs() < 0.001,
        "padding_bottom must survive"
    );
    assert!(
        (pp.padding_left.map_or(0.0, |p| p.value()) - 6.0).abs() < 0.001,
        "padding_left must survive"
    );
    assert!(
        (pp.padding_right.map_or(0.0, |p| p.value()) - 7.0).abs() < 0.001,
        "padding_right must survive"
    );
}
