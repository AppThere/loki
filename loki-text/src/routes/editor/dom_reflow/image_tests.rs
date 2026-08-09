// SPDX-License-Identifier: Apache-2.0

//! Tests for T7.3's *scalable* oversized element.
//!
//! These are the DOM statement of `loki_layout::flow_para_images::fit_to_column`,
//! and they are written against the same three mutations its own tests kill:
//! scaling the width without the height, scaling *up*, and fitting where the
//! view should not fit.

use super::img_css;

/// The value of declaration `prop`, matched as a **property** rather than as a
/// substring: `contains("width:")` also matches inside `max-width:`, which is
/// how the first version of the "never scales up" test below failed on correct
/// output.
fn decl<'a>(css: &'a str, prop: &str) -> Option<&'a str> {
    css.split(';')
        .map(str::trim)
        .filter_map(|d| d.split_once(':'))
        .find(|(p, _)| p.trim() == prop)
        .map(|(_, v)| v.trim())
}

/// **Fitted, not squashed** — and to the *declared* ratio.
///
/// A width without a ratio scales one axis and leaves the other, which distorts
/// the image: the first mutation `fit_to_column`'s own tests kill. The ratio is
/// the declared one because that is what `fit_to_column` scales; taking the
/// file's instead would render a stretched image square here and oblong on the
/// canvas.
#[test]
fn the_height_follows_the_declared_ratio() {
    let css = img_css(576.0, 144.0);
    assert_eq!(decl(&css, "aspect-ratio"), Some("576 / 144"), "{css}");
    assert_eq!(decl(&css, "height"), None, "a second height rule: {css}");
}

/// With no declared height there is no declared ratio, so the file's own is the
/// only one available — the inversion, and the case a plain `aspect-ratio`
/// would divide by zero on.
#[test]
fn an_image_with_no_declared_height_uses_the_files_ratio() {
    let css = img_css(400.0, 0.0);
    assert_eq!(decl(&css, "height"), Some("auto"), "{css}");
    assert_eq!(decl(&css, "aspect-ratio"), None, "{css}");
}

/// **It never scales up.** The natural width is a `max-width`, so an image
/// narrower than the column keeps its own size; making it `width: 400pt`
/// outright would stretch every small figure across the measure.
#[test]
fn a_small_image_is_not_stretched_to_the_column() {
    let css = img_css(120.0, 60.0);
    assert_eq!(decl(&css, "max-width"), Some("120pt"), "{css}");
    // `width` is the percentage, **not** the natural size: setting the natural
    // size outright leaves nothing to shrink, and `contains` would not catch it
    // because `max-width: 120pt` carries the same characters.
    assert_eq!(
        decl(&css, "width"),
        Some("100%"),
        "the natural width was set outright, so the image cannot shrink: {css}"
    );
}

/// The cap is the image's own width, so two images cap differently. A constant
/// would pass both tests above and fit every figure to the same box.
#[test]
fn the_cap_is_the_images_own_width() {
    assert_ne!(img_css(120.0, 60.0), img_css(400.0, 200.0));
    assert!(img_css(400.0, 200.0).contains("400pt"));
}

/// Block-level, because the canvas path stacks images above the paragraph rather
/// than inline (`TODO(inline-image-flow)`: Parley has no inline image box). An
/// inline image here would sit in a different place from the same image there,
/// and the two views would disagree for a reason that is not rendering.
#[test]
fn an_image_is_block_level_like_the_canvas_paths_stack() {
    assert_eq!(decl(&img_css(400.0, 200.0), "display"), Some("block"));
}
