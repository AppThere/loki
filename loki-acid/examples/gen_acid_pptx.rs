// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Generates `loki-acid/assets/acid_pptx.pptx`, the PPTX acid fixture.
//!
//! Run with `cargo run -p loki-acid --example gen_acid_pptx`.
//!
//! # Provenance — read before trusting this fixture
//!
//! Unlike `acid_docx.docx` / `acid_odt.odt` (authored in real Office / LibreOffice
//! to test *import* fidelity against third-party output), this deck is written by
//! Loki's **own** `PptxExport`. It is therefore a **round-trip** fixture: it
//! covers only constructs Loki can already emit (text boxes with run formatting,
//! preset geometries with fill/stroke, placeholders, multiple slides). It does
//! **not** exercise the harder catalogued cases (gradients, SmartArt, charts,
//! animations, grouped-shape child transforms), which need a PowerPoint-authored
//! deck. Treat it as a placeholder that unblocks the import/pagination canaries
//! for the 29 PPTX cases, not as a fidelity oracle for them.

use std::io::Cursor;
use std::path::Path;

use loki_graphics::{
    Fill, Geometry, PresetShape, RectF, Shape, Stroke, TextAlign, TextBody, TextParagraph, TextRun,
    TextRunProps,
};
use loki_ooxml::pptx::export::PptxExport;
use loki_presentation_model::{PlaceholderKind, Presentation, Slide};
use loki_primitives::color::DocumentColor;

fn color(hex: &str) -> DocumentColor {
    DocumentColor::from_hex(hex).expect("valid hex")
}

/// A title slide: a centred, bold, coloured title text box + a placeholder.
fn title_slide(p: &Presentation) -> Slide {
    let mut slide = Slide::new("slide1", p.slide_size);
    let body = TextBody {
        paragraphs: vec![TextParagraph {
            runs: vec![TextRun {
                text: "Loki ACID — Presentation".to_string(),
                props: TextRunProps {
                    bold: true,
                    font_size_pt: Some(40.0),
                    color: Some(color("#1A1A2E")),
                    ..Default::default()
                },
            }],
            align: TextAlign::Center,
            level: 0,
        }],
        ..Default::default()
    };
    let mut title = Shape::text_box("2", RectF::new(60.0, 40.0, 840.0, 120.0), body);
    title.name = Some("Title 1".to_string());
    slide.drawing.push(title);
    slide.add_placeholder(PlaceholderKind::Title, "2");
    slide
}

/// A content slide: a left-aligned bulleted body plus a filled, stroked shape.
fn content_slide(p: &Presentation) -> Slide {
    let mut slide = Slide::new("slide2", p.slide_size);

    let body = TextBody {
        paragraphs: vec![
            TextParagraph {
                runs: vec![TextRun {
                    text: "First point".to_string(),
                    props: TextRunProps::default(),
                }],
                align: TextAlign::Left,
                level: 0,
            },
            TextParagraph {
                runs: vec![TextRun {
                    text: "Second point, indented".to_string(),
                    props: TextRunProps {
                        italic: true,
                        ..Default::default()
                    },
                }],
                align: TextAlign::Left,
                level: 1,
            },
        ],
        ..Default::default()
    };
    let mut content = Shape::text_box("2", RectF::new(60.0, 60.0, 480.0, 360.0), body);
    content.name = Some("Content".to_string());
    slide.drawing.push(content);

    let rect = Shape::geometry(
        "3",
        RectF::new(580.0, 120.0, 300.0, 200.0),
        Geometry::Preset(PresetShape::Rectangle),
        Fill::Solid(color("#0F3460")),
        Some(Stroke::solid(color("#E94560"), 3.0)),
    );
    slide.drawing.push(rect);
    slide
}

/// A shapes slide: a couple of preset geometries with different fills.
fn shapes_slide(p: &Presentation) -> Slide {
    let mut slide = Slide::new("slide3", p.slide_size);
    let ellipse = Shape::geometry(
        "2",
        RectF::new(80.0, 100.0, 240.0, 240.0),
        Geometry::Preset(PresetShape::Ellipse),
        Fill::Solid(color("#16C79A")),
        Some(Stroke::solid(color("#11999E"), 2.0)),
    );
    slide.drawing.push(ellipse);

    let triangle = Shape::geometry(
        "3",
        RectF::new(420.0, 100.0, 240.0, 240.0),
        Geometry::Preset(PresetShape::Triangle),
        Fill::Solid(color("#FFD460")),
        None,
    );
    slide.drawing.push(triangle);
    slide
}

fn build_deck() -> Presentation {
    let mut p = Presentation::new(Presentation::WIDESCREEN_16_9);
    p.meta.title = Some("Loki ACID Presentation Fixture".to_string());
    p.meta.author = Some("Loki ACID harness".to_string());

    let s1 = title_slide(&p);
    let s2 = content_slide(&p);
    let s3 = shapes_slide(&p);
    p.add_slide(s1);
    p.add_slide(s2);
    p.add_slide(s3);
    p
}

fn main() {
    let deck = build_deck();

    let mut buf = Cursor::new(Vec::new());
    PptxExport::export(&deck, &mut buf).expect("export acid pptx");
    let bytes = buf.into_inner();

    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/acid_pptx.pptx");
    std::fs::write(&out, &bytes).expect("write fixture");
    println!(
        "wrote {} ({} bytes, {} slides)",
        out.display(),
        bytes.len(),
        deck.slide_count()
    );
}
