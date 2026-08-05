// SPDX-License-Identifier: Apache-2.0

//! A paragraph's images, for the DOM reflow view — T7.3's *scalable* oversized
//! element, next to the table's unscalable one.
//!
//! # Fit to the column, aspect preserved — the same rule, said in CSS
//!
//! The canvas path scales an oversized image down with
//! `loki_layout::flow_para_images::fit_to_column`, and only in reflow mode
//! (`LayoutMode::fits_oversized_to_column`) — the paginated view is a fidelity
//! view of a page with a real width, where Word and LibreOffice let an oversized
//! image overhang and matching them is the point.
//!
//! Here the same rule is `width: 100%; max-width: <natural>`:
//!
//! * in the column, `100%` is the column and the cap is the image's own size, so
//!   it shrinks **only** when it is too wide — never scales up, which is
//!   `fit_to_column`'s first clause;
//! * `height: auto` keeps the aspect, which is its second;
//! * inside [`super::oversized::AtOversized`]'s expanded state the parent is
//!   `max-content`, so `100%` resolves to the cap and the image is at its
//!   natural size while *that box* scrolls.
//!
//! Stated as two declarations rather than as a computed width, so the fitting is
//! done by the layout that paints it. A width computed here would be a second
//! copy of the arithmetic, resolved against a column measured a frame earlier.
//!
//! # Where the pixels come from — and that they do not arrive yet
//!
//! `CollectedImage::src` is already a `data:` URI (the resolver builds it), and
//! the vendored `blitz-net` provider decodes `data:`, so an `<img>` needs no
//! media plumbing of its own.
//!
//! **Measured, and it does not paint.** In the `oversized` sitting scenario the
//! box is laid out in the right place at the right size — confirmed by giving
//! the element a background, which appears exactly where the figure should be —
//! and the bitmap never lands. So the geometry T7.3 is about is right and the
//! decode is not established. `TODO(dom-reflow-image-pixels)`.
//!
//! # Not covered
//!
//! **Floating images.** `CollectedImage::float` carries the wrap mode, and this
//! renders every image as a block-level prefix above the paragraph — which is
//! what the canvas path does for non-floating images (`stack_block_images`) and
//! is wrong for a wrapped one. A floated image is rendered in flow and marked,
//! rather than silently placed where it does not belong.
//! `TODO(dom-reflow-float)`. Text boxes (`CollectedImage::textbox`) are not
//! rendered at all.

use dioxus::prelude::*;
use loki_layout::resolve::{CollectedImage, emu_to_pt};

/// One image's declarations at its **declared** size.
///
/// `width: 100%` with the declared width as the cap: shrink to the column, never
/// grow past the image's own size — `fit_to_column`'s two clauses. The third is
/// aspect preservation, and it is `aspect-ratio` rather than `height: auto`
/// because `fit_to_column` scales the **declared** width and height together:
/// `height: auto` would use the file's *intrinsic* ratio instead, so a document
/// that stretches a 2 × 2 image to 8 × 2 inches would render square here and
/// oblong on the canvas.
///
/// It also gives the box a size before the bytes arrive. The `data:` URI is
/// fetched asynchronously, and with `height: auto` an unloaded image is zero
/// tall — measured: the figure was invisible and the geometry under test never
/// happened.
#[must_use]
pub(super) fn img_css(w_pt: f32, h_pt: f32) -> String {
    let ratio = if h_pt > 0.0 {
        format!(" aspect-ratio: {w_pt} / {h_pt};")
    } else {
        // No declared height: fall back to the file's own ratio, which is the
        // only ratio there is.
        " height: auto;".to_string()
    };
    format!("display: block; width: 100%; max-width: {w_pt}pt;{ratio}")
}

/// The images collected from one paragraph, as block-level elements above it.
///
/// Empty when the paragraph has none, which is the common case — so this costs
/// an `is_empty` on almost every paragraph and nothing else.
pub(super) fn images_el(images: &[CollectedImage]) -> Element {
    rsx! {
        for (i, image) in images.iter().enumerate() {
            { rsx! { div { key: "{i}", { image_el(image) } } } }
        }
    }
}

/// One image, in its own scrollport.
fn image_el(image: &CollectedImage) -> Element {
    if image.textbox.is_some() {
        return rsx! {
            div {
                style: "border: 1px dashed #c0392b; color: #c0392b; padding: 2pt 4pt; \
                        margin: 4pt 0; font-size: 9pt;",
                "[dom-reflow: text box not rendered]"
            }
        };
    }
    let (w, h) = (emu_to_pt(image.cx_emu), emu_to_pt(image.cy_emu));
    let alt = image.alt.clone().unwrap_or_default();
    // A floated image is placed in flow here, which is not where the document
    // puts it. Marked rather than silently misplaced — an omission that looks
    // like agreement is the failure mode this view must not have.
    let floated = image.float.is_some();
    rsx! {
        super::oversized::AtOversized {
            fittable: true,
            div {
                if floated {
                    div {
                        style: "color: #c0392b; font-size: 9pt;",
                        "[dom-reflow: float ignored]"
                    }
                }
                img {
                    src: "{image.src}",
                    alt: "{alt}",
                    style: img_css(w, h),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
