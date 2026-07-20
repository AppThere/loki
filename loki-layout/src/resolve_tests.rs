// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for [`crate::resolve`].

use super::*;

use appthere_color::RgbColor;
use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::field::types::{Field, FieldKind};
use loki_doc_model::content::inline::{Inline, LinkTarget, NoteKind, StyledRun};
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::{
    CharProps, HighlightColor, StrikethroughStyle as DocStrikethroughStyle,
    UnderlineStyle as DocUnderlineStyle, VerticalAlign as DocVerticalAlign,
};
use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

// ── helpers ───────────────────────────────────────────────────────────────────

fn empty_para(inlines: Vec<Inline>) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines,
        attr: NodeAttr::default(),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn resolve_color_rgb_values() {
    let dc = DocumentColor::Rgb(RgbColor::new(1.0, 0.5, 0.0));
    let lc = resolve_color(Some(&dc));
    assert!((lc.r - 1.0).abs() < 1e-5, "r mismatch");
    assert!((lc.g - 0.5).abs() < 1e-5, "g mismatch");
    assert!(lc.b.abs() < 1e-5, "b mismatch");
    assert!((lc.a - 1.0).abs() < 1e-5, "alpha should be 1.0");
}

#[test]
fn resolve_color_transparent() {
    let lc = resolve_color(Some(&DocumentColor::Transparent));
    assert_eq!(lc, LayoutColor::TRANSPARENT);
}

#[test]
fn resolve_color_none_gives_black() {
    assert_eq!(resolve_color(None), LayoutColor::BLACK);
}

#[test]
fn pts_to_f32_value() {
    let result = pts_to_f32(Points::new(14.5));
    assert!((result - 14.5_f32).abs() < 1e-5);
}

#[test]
fn flatten_plain_str() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Str("hello".into())]);
    let (text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(text, "hello");
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].range, 0..5);
}

#[test]
fn flatten_str_space_str() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![
        Inline::Str("hello".into()),
        Inline::Space,
        Inline::Str("world".into()),
    ]);
    let (text, _spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(text, "hello world");
}

#[test]
fn flatten_strong_sets_bold() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Strong(vec![Inline::Str("bold".into())])]);
    let (text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(text, "bold");
    assert!(!spans.is_empty());
    assert!(spans[0].bold, "Strong should produce bold=true");
}

#[test]
fn revision_display_modes_change_flattened_text_and_decoration() {
    use crate::options::RevisionDisplay;
    use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

    let tracked = |text: &str, kind: RevisionKind| {
        Inline::StyledRun(StyledRun {
            style_id: None,
            direct_props: Some(Box::new(CharProps {
                revision: Some(RevisionMark {
                    kind,
                    author: Some("Ada".into()),
                    date: None,
                    id: Some("1".into()),
                }),
                ..Default::default()
            })),
            content: vec![Inline::Str(text.into())],
            attr: NodeAttr::default(),
        })
    };
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![
        Inline::Str("keep ".into()),
        tracked("new", RevisionKind::Insertion),
        tracked("gone", RevisionKind::Deletion),
    ]);
    let flatten =
        |mode| crate::resolve::flatten_paragraph_with_base(&para, &catalog, &mut 0u32, None, mode);

    // All-Markup: both runs shown; insertion underlined, deletion struck.
    let (text, spans, _, _) = flatten(RevisionDisplay::AllMarkup);
    assert_eq!(text, "keep newgone");
    assert!(
        spans.iter().any(|s| s.underline.is_some()),
        "insertion underlined"
    );
    assert!(
        spans.iter().any(|s| s.strikethrough.is_some()),
        "deletion struck"
    );

    // Final: deletion hidden, insertion shown as normal text (no decoration).
    let (text, spans, _, _) = flatten(RevisionDisplay::Final);
    assert_eq!(text, "keep new");
    assert!(
        spans.iter().all(|s| s.underline.is_none()),
        "no ins underline"
    );
    assert!(spans.iter().all(|s| s.strikethrough.is_none()));

    // Original: insertion hidden, deletion shown as normal text.
    let (text, spans, _, _) = flatten(RevisionDisplay::Original);
    assert_eq!(text, "keep gone");
    assert!(
        spans.iter().all(|s| s.strikethrough.is_none()),
        "no del strike"
    );
}

#[test]
fn flatten_emph_sets_italic() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Emph(vec![Inline::Str("italic".into())])]);
    let (_, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert!(!spans.is_empty());
    assert!(spans[0].italic, "Emph should produce italic=true");
}

#[test]
fn formatting_does_not_leak_to_following_siblings() {
    // Regression for the `walk_inlines` mutate-and-restore optimisation: a plain
    // run *after* a Strong must not inherit bold (the effective props must be
    // restored once the styled run's children are processed).
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![
        Inline::Strong(vec![Inline::Str("a".into())]),
        Inline::Str("b".into()),
    ]);
    let (text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(text, "ab");
    let bold_a = spans
        .iter()
        .find(|s| s.range == (0..1))
        .expect("span for 'a'");
    let plain_b = spans
        .iter()
        .find(|s| s.range == (1..2))
        .expect("span for 'b'");
    assert!(bold_a.bold, "'a' inside Strong must be bold");
    assert!(
        !plain_b.bold,
        "'b' after Strong must NOT be bold (effective props restored)"
    );
}

#[test]
fn nested_formatting_accumulates_and_restores() {
    // Strong( Emph("ab") + "cd" ) + "ef":
    //   "ab" → bold + italic, "cd" → bold only (Emph restored),
    //   "ef" → unformatted (Strong restored).
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![
        Inline::Strong(vec![
            Inline::Emph(vec![Inline::Str("ab".into())]),
            Inline::Str("cd".into()),
        ]),
        Inline::Str("ef".into()),
    ]);
    let (text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(text, "abcdef");
    let ab = spans.iter().find(|s| s.range == (0..2)).expect("span 'ab'");
    let cd = spans.iter().find(|s| s.range == (2..4)).expect("span 'cd'");
    let ef = spans.iter().find(|s| s.range == (4..6)).expect("span 'ef'");
    assert!(ab.bold && ab.italic, "'ab' must be bold + italic");
    assert!(cd.bold && !cd.italic, "'cd' must stay bold but not italic");
    assert!(!ef.bold && !ef.italic, "'ef' must be unformatted");
}

#[test]
fn flatten_styled_run_applies_direct_props() {
    let catalog = StyleCatalog::new();
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            font_size: Some(Points::new(24.0)),
            bold: Some(true),
            ..Default::default()
        })),
        content: vec![Inline::Str("big".into())],
        attr: NodeAttr::default(),
    };
    let para = empty_para(vec![Inline::StyledRun(run)]);
    let (_, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert!(!spans.is_empty());
    assert!(
        (spans[0].font_size - 24.0).abs() < 1e-5,
        "font_size should be 24pt"
    );
    assert!(spans[0].bold, "bold should be true");
}

#[test]
fn resolve_para_props_defaults() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![]);
    let resolved = resolve_para_props(&para, &catalog);
    assert_eq!(resolved.space_before, 0.0);
    assert_eq!(resolved.indent_start, 0.0);
    assert!(!resolved.keep_together);
    assert!(!resolved.page_break_before);
}

#[test]
fn resolve_para_props_center_from_style() {
    let mut catalog = StyleCatalog::new();
    catalog.paragraph_styles.insert(
        StyleId::new("Center"),
        ParagraphStyle {
            id: StyleId::new("Center"),
            display_name: None,
            parent: None,
            linked_char_style: None,
            next_style_id: None,
            para_props: ParaProps {
                alignment: Some(ParagraphAlignment::Center),
                ..Default::default()
            },
            char_props: CharProps::default(),
            is_default: false,
            is_custom: false,
            extensions: ExtensionBag::default(),
        },
    );
    let para = StyledParagraph {
        style_id: Some(StyleId::new("Center")),
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![],
        attr: NodeAttr::default(),
    };
    let resolved = resolve_para_props(&para, &catalog);
    assert_eq!(resolved.alignment, parley::Alignment::Center);
}

#[test]
fn char_props_to_style_span_maps_new_fields() {
    let props = CharProps {
        vertical_align: Some(DocVerticalAlign::Superscript),
        highlight_color: Some(HighlightColor::Yellow),
        letter_spacing: Some(Points::new(2.0)),
        small_caps: Some(true),
        word_spacing: Some(Points::new(3.0)),
        shadow: Some(true),
        underline: Some(DocUnderlineStyle::Double),
        strikethrough: Some(DocStrikethroughStyle::Single),
        ..Default::default()
    };
    let span = char_props_to_style_span(&props, 0..1);

    assert_eq!(
        span.vertical_align,
        Some(crate::para::VerticalAlign::Superscript)
    );
    assert!(
        span.highlight_color.is_some(),
        "highlight_color must be mapped"
    );
    assert!((span.letter_spacing.unwrap() - 2.0).abs() < 1e-5);
    assert_eq!(span.font_variant, Some(crate::para::FontVariant::SmallCaps));
    assert!((span.word_spacing.unwrap() - 3.0).abs() < 1e-5);
    assert!(span.shadow, "shadow must be true");
    assert!(span.underline.is_some(), "underline must be mapped");
    assert!(span.strikethrough.is_some(), "strikethrough must be mapped");
}

#[test]
fn flatten_all_caps_uppercases_text() {
    let catalog = StyleCatalog::new();
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            all_caps: Some(true),
            ..Default::default()
        })),
        content: vec![Inline::Str("hello".into())],
        attr: NodeAttr::default(),
    };
    let para = empty_para(vec![Inline::StyledRun(run)]);
    let (text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(text, "HELLO", "all_caps must uppercase text during flatten");
    assert_eq!(
        spans[0].font_variant,
        Some(crate::para::FontVariant::AllCaps)
    );
}

#[test]
fn flatten_small_caps_uppercases_and_shrinks_lowercase() {
    // Small caps is synthesized: every letter is uppercased, and the letters that
    // were lowercase in the source render at a reduced size. The run therefore
    // splits at the case boundary into separate spans.
    let catalog = StyleCatalog::new();
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            small_caps: Some(true),
            ..Default::default()
        })),
        content: vec![Inline::Str("Hello".into())],
        attr: NodeAttr::default(),
    };
    let para = empty_para(vec![Inline::StyledRun(run)]);
    let (text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(text, "HELLO", "small caps uppercases every letter");
    assert!(
        spans.len() >= 2,
        "the H|ello case boundary must split into >= 2 spans, got {}",
        spans.len()
    );
    // The leading 'H' (already uppercase) keeps the full 12 pt size.
    assert!(
        (spans[0].font_size - 12.0).abs() < 1e-3,
        "uppercase-origin letter stays full size, got {}",
        spans[0].font_size
    );
    // 'ello' (originally lowercase) shrinks to the small-cap ratio (0.8 × 12).
    assert!(
        spans
            .iter()
            .any(|s| (s.font_size - 12.0 * 0.8).abs() < 1e-3),
        "lowercase-origin letters must render at the reduced small-cap size"
    );
}

#[test]
fn flatten_superscript_inline_sets_vertical_align() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Superscript(vec![Inline::Str("2".into())])]);
    let (text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(text, "2");
    assert_eq!(
        spans[0].vertical_align,
        Some(crate::para::VerticalAlign::Superscript),
        "Inline::Superscript must set vertical_align=Superscript"
    );
}

// ── gap #9: inline image collection ──────────────────────────────────────────

#[test]
fn flatten_image_collects_emu_dimensions() {
    let catalog = StyleCatalog::new();
    let mut attr = NodeAttr::default();
    attr.kv.push(("cx_emu".to_string(), "914400".to_string())); // 72 pt
    attr.kv.push(("cy_emu".to_string(), "457200".to_string())); // 36 pt
    let target = LinkTarget::new("data:image/png;base64,ABC");
    let para = empty_para(vec![Inline::Image(attr, vec![], target)]);
    let (_text, _spans, images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(images.len(), 1, "one image must be collected");
    assert_eq!(images[0].src, "data:image/png;base64,ABC");
    assert_eq!(images[0].cx_emu, 914400);
    assert_eq!(images[0].cy_emu, 457200);
    let w = emu_to_pt(images[0].cx_emu);
    let h = emu_to_pt(images[0].cy_emu);
    assert!(
        (w - 72.0).abs() < 0.1,
        "914400 EMU should be ~72 pt, got {w}"
    );
    assert!(
        (h - 36.0).abs() < 0.1,
        "457200 EMU should be ~36 pt, got {h}"
    );
}

#[test]
fn flatten_class_only_floating_image_is_collected_as_float() {
    // An image tagged only with the floating class (no wrap keys — e.g. an
    // anchored DOCX drawing with no wrap child) must be collected as a float,
    // not inline (5.6). Contrast with a plain inline image (float == None).
    use loki_doc_model::content::float::{FLOATING_CLASS, TextWrap, WrapSide};
    let catalog = StyleCatalog::new();
    let mut attr = NodeAttr::default();
    attr.classes.push(FLOATING_CLASS.to_string());
    let target = LinkTarget::new("data:image/png;base64,ABC");
    let para = empty_para(vec![Inline::Image(attr, vec![], target)]);
    let (_text, _spans, images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(images.len(), 1);
    let float = images[0]
        .float
        .expect("floating class → collected as a float");
    assert_eq!(float.wrap, TextWrap::Square);
    assert_eq!(float.side, WrapSide::Both);
    assert!(!float.behind_text);

    // A plain inline image (no class, no wrap keys) stays non-floating.
    let inline = empty_para(vec![Inline::Image(
        NodeAttr::default(),
        vec![],
        LinkTarget::new("data:image/png;base64,ABC"),
    )]);
    let (_t, _s, inline_images, _n) = flatten_paragraph(&inline, &catalog, &mut 0u32);
    assert!(
        inline_images[0].float.is_none(),
        "inline image is not a float"
    );
}

#[test]
fn flatten_image_zero_size_does_not_panic() {
    let catalog = StyleCatalog::new();
    let attr = NodeAttr::default(); // no kv — cx_emu/cy_emu default to 0
    let target = LinkTarget::new("data:image/png;base64,ABC");
    let para = empty_para(vec![Inline::Image(attr, vec![], target)]);
    let (_text, _spans, images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(images.len(), 1, "zero-size image still collected");
    assert_eq!(images[0].cx_emu, 0);
    assert_eq!(images[0].cy_emu, 0);
}

#[test]
fn flatten_image_empty_src_not_collected() {
    let catalog = StyleCatalog::new();
    let attr = NodeAttr::default();
    let target = LinkTarget::new(""); // empty URL
    let para = empty_para(vec![Inline::Image(attr, vec![], target)]);
    let (_text, _spans, images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert!(
        images.is_empty(),
        "image with empty URL must not be collected"
    );
}

#[test]
fn flatten_image_alt_text_captured() {
    let catalog = StyleCatalog::new();
    let attr = NodeAttr::default();
    let alt_inlines = vec![Inline::Str("logo".into())];
    let target = LinkTarget::new("data:image/png;base64,XYZ");
    let para = empty_para(vec![Inline::Image(attr, alt_inlines, target)]);
    let (_text, _spans, images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(
        images[0].alt.as_deref(),
        Some("logo"),
        "alt text must be captured"
    );
}

// ── gap #11: hyperlink URL propagation ───────────────────────────────────────

#[test]
fn flatten_link_sets_link_url_on_spans() {
    let catalog = StyleCatalog::new();
    let target = LinkTarget::new("https://example.com");
    let link_children = vec![Inline::Str("click".into())];
    let para = empty_para(vec![Inline::Link(
        NodeAttr::default(),
        link_children,
        target,
    )]);
    let (_text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert!(!spans.is_empty(), "link must produce at least one span");
    assert_eq!(
        spans[0].link_url.as_deref(),
        Some("https://example.com"),
        "link_url must be set on spans inside Inline::Link"
    );
}

#[test]
fn flatten_link_auto_underlines_when_no_underline() {
    let catalog = StyleCatalog::new();
    let target = LinkTarget::new("https://example.com");
    let link_children = vec![Inline::Str("text".into())];
    let para = empty_para(vec![Inline::Link(
        NodeAttr::default(),
        link_children,
        target,
    )]);
    let (_text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert!(
        spans[0].underline.is_some(),
        "link spans without explicit underline must be auto-underlined"
    );
}

#[test]
fn flatten_non_link_text_has_no_link_url() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Str("plain".into())]);
    let (_text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert!(
        spans[0].link_url.is_none(),
        "non-link spans must not have link_url set"
    );
}

// ── gap #4: field codes ───────────────────────────────────────────────────────

#[test]
fn flatten_field_uses_current_value() {
    let catalog = StyleCatalog::new();
    let field = Field::new(FieldKind::PageNumber).with_current_value("42");
    let para = empty_para(vec![Inline::Field(field)]);
    let (text, _spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(
        text, "42",
        "Field with current_value should emit that value"
    );
}

#[test]
fn flatten_field_page_number_fallback() {
    let catalog = StyleCatalog::new();
    let field = Field::new(FieldKind::PageNumber); // no current_value
    let para = empty_para(vec![Inline::Field(field)]);
    let (text, _spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert_eq!(
        text, "1",
        "PageNumber without current_value should fall back to \"1\""
    );
}

#[test]
fn flatten_field_raw_no_current_value_emits_nothing() {
    let catalog = StyleCatalog::new();
    let field = Field::new(FieldKind::Raw {
        instruction: "HYPERLINK".into(),
    });
    let para = empty_para(vec![Inline::Field(field)]);
    let (text, _spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert!(
        text.is_empty(),
        "Raw field without current_value should emit empty string"
    );
}

// ── gap #2: footnotes ─────────────────────────────────────────────────────────

#[test]
fn flatten_footnote_emits_superscript_mark_and_collects_body() {
    let catalog = StyleCatalog::new();
    let body = vec![Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str("note body".into())],
        attr: NodeAttr::default(),
    })];
    let para = empty_para(vec![
        Inline::Str("text".into()),
        Inline::Note(NoteKind::Footnote, body.clone()),
    ]);
    let mut counter = 0u32;
    let (text, _spans, _images, notes) = flatten_paragraph(&para, &catalog, &mut counter);
    assert!(text.contains("text"), "main text must be present");
    assert!(
        text.contains('\u{00B9}'),
        "superscript ¹ must be emitted for note 1"
    );
    assert_eq!(notes.len(), 1, "one note must be collected");
    assert_eq!(notes[0].number, 1);
    assert!(matches!(notes[0].kind, NoteKind::Footnote));
    assert_eq!(counter, 1);
}

#[test]
fn flatten_multiple_footnotes_increment_counter() {
    let catalog = StyleCatalog::new();
    let note1 = Inline::Note(NoteKind::Footnote, vec![]);
    let note2 = Inline::Note(NoteKind::Endnote, vec![]);
    let para = empty_para(vec![note1, note2]);
    let mut counter = 0u32;
    let (text, _spans, _images, notes) = flatten_paragraph(&para, &catalog, &mut counter);
    assert_eq!(counter, 2);
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].number, 1);
    assert_eq!(notes[1].number, 2);
    // ¹ = U+00B9, ² = U+00B2
    assert!(text.contains('\u{00B9}'));
    assert!(text.contains('\u{00B2}'));
}

#[test]
fn flatten_footnote_mark_superscript_vertical_align() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Note(NoteKind::Footnote, vec![])]);
    let (text, spans, _images, _notes) = flatten_paragraph(&para, &catalog, &mut 0u32);
    assert!(!text.is_empty(), "mark must be emitted");
    let mark_span = spans
        .iter()
        .find(|s| s.vertical_align == Some(crate::para::VerticalAlign::Superscript));
    assert!(
        mark_span.is_some(),
        "note mark span must have Superscript vertical_align"
    );
}

// ── character-style chain cycle tests ─────────────────────────────────────────

/// Build a catalog whose character styles form a parent cycle:
/// A.parent = B, B.parent = A.
fn cyclic_char_style_catalog() -> StyleCatalog {
    use loki_doc_model::style::char_style::CharacterStyle;
    let mut catalog = StyleCatalog::new();
    let mk = |id: &str, parent: &str, props: CharProps| CharacterStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: Some(StyleId::new(parent)),
        char_props: props,
        extensions: ExtensionBag::default(),
    };
    catalog.character_styles.insert(
        StyleId::new("A"),
        mk(
            "A",
            "B",
            CharProps {
                bold: Some(true),
                ..Default::default()
            },
        ),
    );
    catalog.character_styles.insert(
        StyleId::new("B"),
        mk(
            "B",
            "A",
            CharProps {
                italic: Some(true),
                ..Default::default()
            },
        ),
    );
    catalog
}

#[test]
fn char_style_chain_cycle_terminates_without_overflow() {
    let catalog = cyclic_char_style_catalog();
    let run = StyledRun {
        style_id: Some(StyleId::new("A")),
        direct_props: None,
        content: vec![Inline::Str("cyclic".into())],
        attr: NodeAttr::default(),
    };
    // Must not overflow the stack; own value wins, parent value inherited.
    let span = resolve_char_props(&run, &catalog, &CharProps::default());
    assert!(span.bold, "A's own bold must survive");
    assert!(
        span.italic,
        "B's italic must be inherited through the cycle"
    );
}

#[test]
fn char_style_self_referential_parent_terminates() {
    use loki_doc_model::style::char_style::CharacterStyle;
    let mut catalog = StyleCatalog::new();
    catalog.character_styles.insert(
        StyleId::new("Loop"),
        CharacterStyle {
            id: StyleId::new("Loop"),
            display_name: None,
            parent: Some(StyleId::new("Loop")),
            char_props: CharProps {
                bold: Some(true),
                ..Default::default()
            },
            extensions: ExtensionBag::default(),
        },
    );
    let props = char_span::resolve_char_style_chain(&catalog, &StyleId::new("Loop"));
    assert_eq!(props.bold, Some(true));
    assert!(props.italic.is_none());
}
