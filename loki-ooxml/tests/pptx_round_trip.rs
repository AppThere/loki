// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration round-trip tests for PPTX export/import (audit T-3).
//!
//! The inline `pptx/export_tests.rs` suite covers a single-slide deck; this file
//! exercises a **multi-slide** presentation through the full
//! model → export → re-import cycle and re-opens the OPC package to confirm one
//! slide part per slide plus a well-formed content-types stream. Gated on the
//! `pptx` feature (run with `cargo test -p loki-ooxml --features pptx`).
#![cfg(feature = "pptx")]

use std::io::Cursor;

use loki_graphics::{RectF, Shape, ShapeKind, TextAlign, TextBody};
use loki_ooxml::pptx::export::PptxExport;
use loki_ooxml::pptx::import::{PptxImport, PptxImportOptions};
use loki_presentation_model::{Presentation, Slide};

/// A three-slide deck, each slide carrying a single titled text box whose body
/// is `"Slide N"`, so ordering is checkable after re-import.
fn deck() -> Presentation {
    let mut p = Presentation::new(Presentation::WIDESCREEN_16_9);
    p.meta.title = Some("Three Slides".to_string());

    for n in 1..=3 {
        let mut slide = Slide::new(format!("slide{n}"), p.slide_size);
        let body = TextBody::plain(format!("Slide {n}"));
        let mut tb = Shape::text_box(
            (n + 1).to_string(),
            RectF::new(40.0, 40.0, 880.0, 120.0),
            body,
        );
        tb.name = Some(format!("Title {n}"));
        slide.drawing.push(tb);
        p.add_slide(slide);
    }
    p
}

fn export_then_import(p: &Presentation) -> Presentation {
    let mut buf = Cursor::new(Vec::new());
    PptxExport::export(p, &mut buf).expect("export");
    PptxImport::import(Cursor::new(buf.into_inner()), PptxImportOptions::default())
        .expect("re-import")
}

/// Pulls the first run of text out of a slide's first shape, if any.
fn first_text(slide: &Slide) -> Option<String> {
    let shape = slide.drawing.shapes.first()?;
    let ShapeKind::Geometry(g) = &shape.kind else {
        return None;
    };
    let body = g.text.as_ref()?;
    Some(body.paragraphs.first()?.text())
}

#[test]
fn multi_slide_count_and_order_survive_round_trip() {
    let re = export_then_import(&deck());
    assert_eq!(re.slide_count(), 3, "all three slides must survive");

    for (i, slide) in re.slides.iter().enumerate() {
        let expected = format!("Slide {}", i + 1);
        assert_eq!(
            first_text(slide).as_deref(),
            Some(expected.as_str()),
            "slide {i} text/order changed across the round trip"
        );
    }
}

#[test]
fn slide_size_and_alignment_survive_round_trip() {
    let re = export_then_import(&deck());
    assert!((re.slide_size.width - 960.0).abs() < 1e-6);
    assert!((re.slide_size.height - 540.0).abs() < 1e-6);

    // `TextBody::plain` defaults to left alignment; it must not mutate.
    let slide = &re.slides[0];
    let ShapeKind::Geometry(g) = &slide.drawing.shapes[0].kind else {
        panic!("expected geometry shape");
    };
    let para = &g.text.as_ref().unwrap().paragraphs[0];
    assert_eq!(para.align, TextAlign::Left);
}

#[test]
fn second_round_trip_is_idempotent() {
    let once = export_then_import(&deck());
    let twice = export_then_import(&once);
    assert_eq!(once, twice, "a stable deck must reach a fixed point");
}

#[test]
fn exported_package_has_one_part_per_slide() {
    use loki_opc::{Package, PartName};

    let mut buf = Cursor::new(Vec::new());
    PptxExport::export(&deck(), &mut buf).expect("export");
    let pkg = Package::open(Cursor::new(buf.into_inner())).expect("open package");

    for n in 1..=3 {
        let path = format!("/ppt/slides/slide{n}.xml");
        assert!(
            pkg.part(&PartName::new(&path).unwrap()).is_some(),
            "missing {path}"
        );
    }
    // A fourth slide part must NOT exist (no stray parts from a 3-slide deck).
    assert!(
        pkg.part(&PartName::new("/ppt/slides/slide4.xml").unwrap())
            .is_none(),
        "unexpected extra slide part"
    );

    // presentation.xml must list every slide in order via its relationships.
    let pres = PartName::new("/ppt/presentation.xml").unwrap();
    let rels = pkg.part_relationships(&pres).expect("presentation rels");
    let slide_rels = rels
        .iter()
        .filter(|r| r.rel_type.ends_with("/slide"))
        .count();
    assert_eq!(
        slide_rels, 3,
        "presentation must reference all three slides"
    );
}

#[test]
fn empty_deck_round_trips_to_zero_slides() {
    let p = Presentation::new(Presentation::STANDARD_4_3);
    let re = export_then_import(&p);
    assert_eq!(re.slide_count(), 0);
    assert!((re.slide_size.width - 720.0).abs() < 1e-6);
}
