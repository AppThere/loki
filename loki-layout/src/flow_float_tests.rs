// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`crate::flow::float_impl`].

use super::*;
use loki_doc_model::content::float::{FloatAlign, FloatWrap, TextWrap, WrapSide};

/// One inch = 914400 EMU; build an image of `w_in` × `h_in` inches.
fn img(w_in: f64, h_in: f64, float: Option<FloatWrap>) -> CollectedImage {
    CollectedImage {
        src: "data:image/png;base64,AAAA".into(),
        alt: None,
        cx_emu: (w_in * 914_400.0) as u64,
        cy_emu: (h_in * 914_400.0) as u64,
        float,
        textbox: None,
    }
}

fn square(side: WrapSide) -> FloatWrap {
    FloatWrap {
        wrap: TextWrap::Square,
        side,
        align: None,
        behind_text: false,
    }
}

/// An explicit `wp:positionH` decides which side the float sits on; the wrap
/// side is only consulted when the producer stated no position.
///
/// `wrapText` says which sides *text* may occupy, which pins the object only
/// for `left`/`right`. `bothSides` constrains nothing, and Loki read it as "put
/// the float left" — so `acid2-docx.docx`'s newsletter figure, which is
/// `bothSides` with `<wp:align>right</wp:align>`, came out on the left and
/// mirror-imaged the page against Word. Word's own body copy on that page
/// reads "text flows around the sidebar image on its left".
#[test]
fn an_explicit_position_beats_the_inferred_wrap_side() {
    let plan = |fw: FloatWrap| {
        let images = vec![img(1.0, 1.0, Some(fw))];
        let (_, p) = plan_float(&images, 468.0).expect("a square float is planned");
        // A left float indents the text's start; a right float indents its end.
        (p.indent_start_delta > 0.0, p.indent_end_delta > 0.0)
    };
    let with_align = |side, align| FloatWrap {
        wrap: TextWrap::Square,
        side,
        align,
        behind_text: false,
    };

    // The regressing case: `bothSides` + an explicit right position.
    assert_eq!(
        plan(with_align(WrapSide::Both, Some(FloatAlign::Right))),
        (false, true),
        "an explicit `right` position must put the float on the right"
    );
    // …and the same wrap side with no position keeps the old inference, which
    // is all ODF and legacy content offers.
    assert_eq!(
        plan(with_align(WrapSide::Both, None)),
        (true, false),
        "with no stated position, `bothSides` still falls back to a left float"
    );
    // The position must also be able to *override* a wrap side that would have
    // inferred the opposite, or it is not really deciding.
    assert_eq!(
        plan(with_align(WrapSide::Left, Some(FloatAlign::Left))),
        (true, false),
        "an explicit `left` must win over `wrapText=left`, which infers right"
    );
    assert_eq!(
        plan(with_align(WrapSide::Right, Some(FloatAlign::Right))),
        (false, true),
        "an explicit `right` must win over `wrapText=right`, which infers left"
    );
    // Inference unchanged where the wrap side does pin the object.
    assert_eq!(plan(with_align(WrapSide::Left, None)), (false, true));
    assert_eq!(plan(with_align(WrapSide::Right, None)), (true, false));
}

#[test]
fn inline_image_is_not_planned() {
    let images = vec![img(1.0, 1.0, None)];
    assert!(plan_float(&images, 468.0).is_none());
}

#[test]
fn top_and_bottom_float_is_not_side_wrapped() {
    let images = vec![img(
        1.0,
        1.0,
        Some(FloatWrap {
            wrap: TextWrap::TopAndBottom,
            side: WrapSide::Both,
            align: None,
            behind_text: false,
        }),
    )];
    assert!(plan_float(&images, 468.0).is_none());
}

#[test]
fn wrap_none_is_not_side_wrapped() {
    // Word reserves no space for a `wrapNone` object — the text flows at full
    // width and the image overlaps it. So `plan_float` declines it (the caller
    // emits it as an overlay instead), whether it is in front of or behind text.
    for behind_text in [false, true] {
        let images = vec![img(
            1.0,
            1.0,
            Some(FloatWrap {
                wrap: TextWrap::None,
                side: WrapSide::Both,
                align: None,
                behind_text,
            }),
        )];
        assert!(
            plan_float(&images, 468.0).is_none(),
            "wrapNone never reserves a wrap band (behind_text={behind_text})"
        );
    }
}

#[test]
fn behind_text_float_is_not_side_wrapped() {
    let images = vec![img(
        1.0,
        1.0,
        Some(FloatWrap {
            wrap: TextWrap::Square,
            side: WrapSide::Both,
            align: None,
            behind_text: true,
        }),
    )];
    assert!(plan_float(&images, 468.0).is_none());
}

#[test]
fn side_left_text_puts_float_on_the_right() {
    // WrapSide::Left = text on the left → float on the RIGHT.
    let images = vec![img(1.0, 1.0, Some(square(WrapSide::Left)))];
    let (idx, p) = plan_float(&images, 468.0).expect("planned");
    assert_eq!(idx, 0);
    assert_eq!(p.indent_start_delta, 0.0);
    assert!(p.indent_end_delta > 72.0, "right band ≥ image width + gap");
    if let PositionedItem::Image(im) = &p.item {
        // Right float sits near the right edge.
        assert!(im.rect.origin.x > 468.0 - 80.0);
    } else {
        panic!("expected image item");
    }
}

#[test]
fn side_right_text_puts_float_on_the_left() {
    // WrapSide::Right = text on the right → float on the LEFT.
    let images = vec![img(1.0, 1.0, Some(square(WrapSide::Right)))];
    let (_, p) = plan_float(&images, 468.0).expect("planned");
    assert!(p.indent_start_delta > 72.0, "left band ≥ image width + gap");
    assert_eq!(p.indent_end_delta, 0.0);
    if let PositionedItem::Image(im) = &p.item {
        assert_eq!(im.rect.origin.x, 0.0, "left float at content origin");
        assert!((im.rect.size.height - 72.0).abs() < 0.5, "1in = 72pt tall");
    } else {
        panic!("expected image item");
    }
}

#[test]
fn both_sides_default_to_a_left_float() {
    let images = vec![img(1.0, 1.0, Some(square(WrapSide::Both)))];
    let (_, p) = plan_float(&images, 468.0).expect("planned");
    assert!(p.indent_start_delta > 0.0, "Both → float left, text right");
    assert_eq!(p.indent_end_delta, 0.0);
}

#[test]
fn oversized_float_is_skipped() {
    // A float wider than 75% of the column leaves too little text width.
    let images = vec![img(6.0, 1.0, Some(square(WrapSide::Both)))];
    assert!(plan_float(&images, 468.0).is_none());
}
