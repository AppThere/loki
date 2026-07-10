// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Layout-geometry assertions for list hanging indent.
//!
//! These are GPU-free, deterministic tests that import a synthetic list
//! paragraph, run `flow_section`, and assert the *x-position* of wrapped
//! continuation lines. They guard the Word/LibreOffice contract that the
//! second and later lines of a wrapping list item align under the **text
//! start** (the hanging indent), not under the bullet/number marker.
//!
//! Symptom this pins: a regression where continuation lines collapse left
//! toward the marker instead of staying at `indent_start`.

use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::Section;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};
use loki_doc_model::style::props::para_props::ParaProps;
use loki_primitives::units::Points;

use loki_layout::{
    FlowOutput, FontResources, LayoutMode, LayoutOptions, PositionedItem, flow_section,
};

const INDENT_START: f64 = 36.0;
const HANGING: f64 = 18.0;

fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

/// A catalog with one bullet list ("b") and one decimal list ("d"), each a
/// single level at `indent_start = 36pt`, `hanging = 18pt` — i.e. the marker
/// hangs at x≈18 and the text starts at x≈36.
fn list_catalog() -> StyleCatalog {
    let mut catalog = StyleCatalog::new();
    catalog.list_styles.insert(
        ListId::new("b"),
        ListStyle {
            id: ListId::new("b"),
            display_name: None,
            levels: vec![ListLevel {
                level: 0,
                kind: ListLevelKind::Bullet {
                    char: BulletChar::Char('•'),
                    font: None,
                },
                indent_start: Points::new(INDENT_START),
                hanging_indent: Points::new(HANGING),
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: None,
                char_props: Default::default(),
            }],
            extensions: ExtensionBag::default(),
        },
    );
    catalog.list_styles.insert(
        ListId::new("d"),
        ListStyle {
            id: ListId::new("d"),
            display_name: None,
            levels: vec![ListLevel {
                level: 0,
                kind: ListLevelKind::Numbered {
                    scheme: NumberingScheme::Decimal,
                    start_value: 1,
                    format: "%1.".to_string(),
                    display_levels: 1,
                },
                indent_start: Points::new(INDENT_START),
                hanging_indent: Points::new(HANGING),
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: None,
                char_props: Default::default(),
            }],
            extensions: ExtensionBag::default(),
        },
    );
    catalog
}

fn list_para(text: &str, list_id: &str) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(ParaProps {
            list_id: Some(ListId::new(list_id)),
            list_level: Some(0),
            indent_start: Some(Points::new(INDENT_START)),
            indent_hanging: Some(Points::new(HANGING)),
            ..Default::default()
        })),
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    }
}

/// A narrow page (200×400pt, 10pt L/R margins → 180pt content) so that a
/// medium-length list item is guaranteed to wrap to several lines, leaving the
/// list's text column ~144pt wide.
fn narrow_layout() -> PageLayout {
    PageLayout {
        page_size: PageSize {
            width: Points::new(200.0),
            height: Points::new(400.0),
        },
        margins: PageMargins {
            top: Points::new(10.0),
            bottom: Points::new(10.0),
            left: Points::new(10.0),
            right: Points::new(10.0),
            ..PageMargins::default()
        },
        ..PageLayout::default()
    }
}

/// (x, y) origin of every glyph run, in layout order.
fn glyph_origins(items: &[PositionedItem]) -> Vec<(f32, f32)> {
    items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(run) => Some((run.origin.x, run.origin.y)),
            _ => None,
        })
        .collect()
}

fn flow(catalog: &StyleCatalog, para: StyledParagraph) -> Vec<PositionedItem> {
    let mut r = test_resources();
    let section = Section::with_layout_and_blocks(narrow_layout(), vec![Block::StyledPara(para)]);
    match flow_section(
        &mut r,
        &section,
        catalog,
        &LayoutMode::Pageless,
        1.0,
        &LayoutOptions::default(),
        &[],
    ) {
        FlowOutput::Canvas { items, .. } => items,
        _ => panic!("expected Canvas output"),
    }
}

/// Split glyph-run origins into the first visual line and the continuation
/// lines, returning `(first_line_min_x, continuation_min_x)`. Panics if the
/// content did not wrap (so a non-wrapping fixture fails loudly rather than
/// silently passing).
fn first_and_continuation_x(origins: &[(f32, f32)]) -> (f32, f32) {
    assert!(!origins.is_empty(), "expected glyph runs");
    let min_y = origins
        .iter()
        .map(|(_, y)| *y)
        .fold(f32::INFINITY, f32::min);
    // Runs on the first line share (approximately) the smallest baseline y.
    let first_line_x = origins
        .iter()
        .filter(|(_, y)| (*y - min_y).abs() < 1.0)
        .map(|(x, _)| *x)
        .fold(f32::INFINITY, f32::min);
    let continuation: Vec<f32> = origins
        .iter()
        .filter(|(_, y)| *y > min_y + 1.0)
        .map(|(x, _)| *x)
        .collect();
    assert!(
        !continuation.is_empty(),
        "fixture must wrap to >=2 lines for this assertion to be meaningful"
    );
    let continuation_x = continuation.iter().cloned().fold(f32::INFINITY, f32::min);
    (first_line_x, continuation_x)
}

#[test]
fn bullet_continuation_aligns_with_hanging_text_start() {
    let catalog = list_catalog();
    let items = flow(
        &catalog,
        list_para(
            "This is a fairly long bullet item whose text must wrap across \
             several lines so we can check where the continuation lines start.",
            "b",
        ),
    );
    let (first_line_x, continuation_x) = first_and_continuation_x(&glyph_origins(&items));

    // The marker hangs to the left, so the first line starts further left than
    // the continuation lines.
    assert!(
        continuation_x > first_line_x + 1.0,
        "continuation lines ({continuation_x}) must be indented to the right of \
         the marker/first line ({first_line_x})"
    );
    // Continuation lines should sit exactly one hanging-indent (18pt) to the
    // right of the marker — i.e. under the text start, matching Word.
    let delta = continuation_x - first_line_x;
    assert!(
        (delta - HANGING as f32).abs() < 2.0,
        "continuation should indent one hanging ({HANGING}pt) past the marker; \
         got {delta}pt (first_line_x={first_line_x}, continuation_x={continuation_x})"
    );
}

/// Extend a catalog with a picture-bullet list "p" (image `src`) at the same
/// geometry as the other lists.
fn with_picture_bullet(mut catalog: StyleCatalog, src: &str) -> StyleCatalog {
    catalog.list_styles.insert(
        ListId::new("p"),
        ListStyle {
            id: ListId::new("p"),
            display_name: None,
            levels: vec![ListLevel {
                level: 0,
                kind: ListLevelKind::Bullet {
                    char: BulletChar::Image {
                        src: src.to_string(),
                    },
                    font: None,
                },
                indent_start: Points::new(INDENT_START),
                hanging_indent: Points::new(HANGING),
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: None,
                char_props: Default::default(),
            }],
            extensions: ExtensionBag::default(),
        },
    );
    catalog
}

#[test]
fn picture_bullet_emits_image_in_the_hanging_label_box() {
    const SRC: &str = "data:image/png;base64,AAAA";
    let catalog = with_picture_bullet(list_catalog(), SRC);
    let items = flow(
        &catalog,
        list_para(
            "This is a fairly long picture-bullet item whose text must wrap \
             across several lines so the text start is unambiguous.",
            "p",
        ),
    );

    // The picture bullet is emitted as an image carrying the level's src.
    let img = items
        .iter()
        .find_map(|i| match i {
            PositionedItem::Image(im) => Some(im),
            _ => None,
        })
        .expect("a picture bullet must emit an image");
    assert_eq!(img.src, SRC);

    // Square, and no wider than the label box (the hanging indent).
    let (w, h) = (img.rect.size.width, img.rect.size.height);
    assert!(
        w > 0.0 && (w - h).abs() < 0.5,
        "bullet is a positive square"
    );
    assert!(
        w <= HANGING as f32 + 0.5,
        "bullet fits the {HANGING}pt label box"
    );

    // Left edge sits one hanging-indent left of the wrapped text start
    // (continuation lines begin exactly at `indent_start`).
    let (_first, continuation_x) = first_and_continuation_x(&glyph_origins(&items));
    let delta = continuation_x - img.rect.origin.x;
    assert!(
        (delta - HANGING as f32).abs() < 2.0,
        "bullet image should sit one hanging ({HANGING}pt) left of the text; got {delta}pt"
    );
}

#[test]
fn numbered_continuation_aligns_with_hanging_text_start() {
    let catalog = list_catalog();
    let items = flow(
        &catalog,
        list_para(
            "This is a fairly long numbered item whose text must wrap across \
             several lines so we can check where the continuation lines start.",
            "d",
        ),
    );
    let (first_line_x, continuation_x) = first_and_continuation_x(&glyph_origins(&items));
    let delta = continuation_x - first_line_x;
    assert!(
        (delta - HANGING as f32).abs() < 2.0,
        "numbered continuation should indent one hanging ({HANGING}pt) past the \
         marker; got {delta}pt"
    );
}
