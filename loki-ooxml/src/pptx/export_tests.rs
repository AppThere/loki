// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Round-trip tests for PPTX export: model → export → import → compare.

use std::io::Cursor;

use super::*;
use crate::pptx::import::{PptxImport, PptxImportOptions};
use loki_graphics::{
    Fill, Geometry, PresetShape, RectF, Shape, ShapeKind, Stroke, TextAlign, TextBody,
    TextParagraph, TextRun, TextRunProps,
};
use loki_presentation_model::{PlaceholderKind, Presentation, Slide};
use loki_primitives::color::DocumentColor;

fn color(hex: &str) -> DocumentColor {
    DocumentColor::from_hex(hex).unwrap()
}

/// A title text box (centered, bold, 44 pt) plus a filled, stroked ellipse.
fn sample() -> Presentation {
    let mut p = Presentation::new(Presentation::WIDESCREEN_16_9);
    p.meta.title = Some("Deck".to_string());

    let mut slide = Slide::new("slide1", p.slide_size);

    let title_body = TextBody {
        paragraphs: vec![TextParagraph {
            runs: vec![TextRun {
                text: "Hello".to_string(),
                props: TextRunProps {
                    bold: true,
                    font_size_pt: Some(44.0),
                    color: Some(color("#112233")),
                    ..Default::default()
                },
            }],
            align: TextAlign::Center,
            level: 0,
        }],
        ..Default::default()
    };
    let mut title = Shape::text_box("2", RectF::new(66.0, 28.75, 828.0, 104.0), title_body);
    title.name = Some("Title 1".to_string());
    slide.drawing.push(title);
    slide.add_placeholder(PlaceholderKind::Title, "2");

    let ellipse = Shape::geometry(
        "3",
        RectF::new(0.0, 0.0, 100.0, 100.0),
        Geometry::Preset(PresetShape::Ellipse),
        Fill::Solid(color("#FF0000")),
        Some(Stroke::solid(color("#0000FF"), 2.0)),
    );
    slide.drawing.push(ellipse);

    p.add_slide(slide);
    p
}

fn export_then_import(p: &Presentation) -> Presentation {
    let mut buf = Cursor::new(Vec::new());
    PptxExport::export(p, &mut buf).expect("export");
    PptxImport::import(Cursor::new(buf.into_inner()), PptxImportOptions::default())
        .expect("re-import")
}

#[test]
fn round_trips_slide_size_and_count() {
    let re = export_then_import(&sample());
    assert_eq!(re.slide_count(), 1);
    assert!((re.slide_size.width - 960.0).abs() < 1e-6);
    assert!((re.slide_size.height - 540.0).abs() < 1e-6);
}

#[test]
fn round_trips_title_shape_text_and_placeholder() {
    let re = export_then_import(&sample());
    let slide = &re.slides[0];
    assert_eq!(slide.drawing.shapes.len(), 2);

    let title = &slide.drawing.shapes[0];
    assert_eq!(title.id.as_str(), "2");
    assert_eq!(title.name.as_deref(), Some("Title 1"));
    assert!((title.transform.frame.x - 66.0).abs() < 1e-6);
    assert!((title.transform.frame.y - 28.75).abs() < 1e-6);

    let ShapeKind::Geometry(g) = &title.kind else {
        panic!("expected geometry");
    };
    let text = g.text.as_ref().unwrap();
    assert_eq!(text.paragraphs[0].text(), "Hello");
    assert_eq!(text.paragraphs[0].align, TextAlign::Center);
    let run = &text.paragraphs[0].runs[0];
    assert!(run.props.bold);
    assert!((run.props.font_size_pt.unwrap() - 44.0).abs() < 1e-6);
    assert_eq!(
        run.props
            .color
            .as_ref()
            .and_then(DocumentColor::to_hex)
            .as_deref(),
        Some("#112233")
    );

    assert_eq!(
        slide
            .placeholder(PlaceholderKind::Title)
            .map(loki_graphics::ShapeId::as_str),
        Some("2")
    );
}

#[test]
fn round_trips_ellipse_fill_and_stroke() {
    let re = export_then_import(&sample());
    let ellipse = &re.slides[0].drawing.shapes[1];
    assert_eq!(ellipse.id.as_str(), "3");
    let ShapeKind::Geometry(g) = &ellipse.kind else {
        panic!("expected geometry");
    };
    assert!(matches!(g.geometry, Geometry::Preset(PresetShape::Ellipse)));
    assert!(matches!(&g.fill, Fill::Solid(c) if c.to_hex().as_deref() == Some("#FF0000")));
    let stroke = g.stroke.as_ref().expect("stroke survives");
    assert!((stroke.width_pt - 2.0).abs() < 1e-6);
    assert_eq!(stroke.color.to_hex().as_deref(), Some("#0000FF"));
}

#[test]
fn round_trips_empty_presentation() {
    let p = Presentation::new(Presentation::STANDARD_4_3);
    let re = export_then_import(&p);
    assert_eq!(re.slide_count(), 0);
    assert!((re.slide_size.width - 720.0).abs() < 1e-6);
}

#[test]
fn idempotent_second_round_trip() {
    let once = export_then_import(&sample());
    let twice = export_then_import(&once);
    assert_eq!(once, twice);
}
