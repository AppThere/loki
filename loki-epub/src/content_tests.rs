// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the EPUB content renderer. Extracted from `content.rs`
//! (inline-test extraction to hold the 300-line ceiling).

use super::*;
use loki_doc_model::content::attr::NodeAttr;

#[test]
fn paragraph_and_heading() {
    let mut doc = Document::new();
    let sec = doc.first_section_mut().unwrap();
    sec.blocks.clear();
    sec.blocks.push(Block::Heading(
        1,
        NodeAttr::default(),
        vec![Inline::Str("Title".into())],
    ));
    sec.blocks
        .push(Block::Para(vec![Inline::Str("Body".into())]));
    let rendered = render_content(&doc);
    assert!(rendered.body.contains("<h1 id=\"h1\">Title</h1>"));
    assert!(rendered.body.contains("<p>Body</p>"));
    assert_eq!(rendered.toc.len(), 1);
    assert_eq!(rendered.toc[0].text, "Title");
}

#[test]
fn math_is_emitted_as_mathml_and_flags_the_document() {
    use loki_doc_model::content::inline::MathType;
    let mut doc = Document::new();
    let sec = doc.first_section_mut().unwrap();
    sec.blocks.clear();
    let mathml = "<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mi>x</mi></math>".to_string();
    sec.blocks.push(Block::Para(vec![
        Inline::Str("Value ".into()),
        Inline::Math(MathType::InlineMath, mathml.clone()),
    ]));
    let rendered = render_content(&doc);
    // MathML is emitted verbatim (not escaped) and the doc is flagged.
    assert!(rendered.body.contains(&mathml), "body: {}", rendered.body);
    assert!(rendered.has_math, "math must flag the content document");
}

#[test]
fn escapes_special_characters() {
    let mut doc = Document::new();
    let sec = doc.first_section_mut().unwrap();
    sec.blocks.clear();
    sec.blocks
        .push(Block::Para(vec![Inline::Str("a < b & c".into())]));
    let rendered = render_content(&doc);
    assert!(rendered.body.contains("a &lt; b &amp; c"));
}

#[test]
fn packages_inline_image() {
    let mut doc = Document::new();
    let sec = doc.first_section_mut().unwrap();
    sec.blocks.clear();
    let target = loki_doc_model::content::inline::LinkTarget::new("data:image/png;base64,SGk=");
    sec.blocks.push(Block::Para(vec![Inline::Image(
        NodeAttr::default(),
        vec![Inline::Str("Alt".into())],
        target,
    )]));
    let rendered = render_content(&doc);
    assert_eq!(rendered.images.len(), 1);
    assert_eq!(rendered.images[0].href, "images/img0.png");
    assert!(
        rendered
            .body
            .contains("<img src=\"images/img0.png\" alt=\"Alt\"/>")
    );
}

#[test]
fn floating_image_emits_css_float() {
    use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};

    let mut doc = Document::new();
    let sec = doc.first_section_mut().unwrap();
    sec.blocks.clear();
    // A left-floating image (text on the right) anchored in a paragraph.
    let mut attr = NodeAttr::default();
    FloatWrap {
        wrap: TextWrap::Square,
        side: WrapSide::Right,
        behind_text: false,
    }
    .store(&mut attr);
    let target = loki_doc_model::content::inline::LinkTarget::new("data:image/png;base64,SGk=");
    sec.blocks.push(Block::Para(vec![
        Inline::Image(attr, vec![Inline::Str("Alt".into())], target),
        Inline::Str("Body text wraps beside the float.".into()),
    ]));
    let rendered = render_content(&doc);
    // Text on the right ⇒ the image floats left so the text wraps around it.
    assert!(
        rendered.body.contains("float:left"),
        "expected a CSS float on the wrapped image; got: {}",
        rendered.body
    );
    assert!(rendered.body.contains("Body text wraps beside the float."));
}

#[test]
fn behind_text_float_is_not_floated() {
    use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};

    let mut doc = Document::new();
    let sec = doc.first_section_mut().unwrap();
    sec.blocks.clear();
    let mut attr = NodeAttr::default();
    FloatWrap {
        wrap: TextWrap::Square,
        side: WrapSide::Both,
        behind_text: true,
    }
    .store(&mut attr);
    let target = loki_doc_model::content::inline::LinkTarget::new("data:image/png;base64,SGk=");
    sec.blocks.push(Block::Para(vec![Inline::Image(
        attr,
        vec![Inline::Str("Alt".into())],
        target,
    )]));
    let rendered = render_content(&doc);
    assert!(
        !rendered.body.contains("float:"),
        "behind-text float must stay block-level"
    );
}
