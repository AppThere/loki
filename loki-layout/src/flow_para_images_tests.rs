// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for [`super`]: block-image alignment and fit-to-column.

#[cfg(test)]
mod align_tests {
    use super::super::stack_block_images;
    use crate::items::PositionedItem;
    use crate::para::ParagraphLayout;
    use crate::resolve::CollectedImage;

    /// 72 pt square (EMU: 914400 per inch).
    fn one_inch_image() -> CollectedImage {
        CollectedImage {
            src: "data:image/png;base64,".into(),
            alt: None,
            cx_emu: 914_400,
            cy_emu: 914_400,
            float: None,
            textbox: None,
        }
    }

    fn empty_layout() -> ParagraphLayout {
        ParagraphLayout {
            height: 0.0,
            width: 0.0,
            items: Vec::new(),
            first_baseline: 0.0,
            last_baseline: 0.0,
            line_boundaries: Vec::new(),
            parley_layout: None,
            orig_to_clean: crate::para::ByteIndexMap::from_indices(&[]),
            clean_to_orig: crate::para::ByteIndexMap::from_indices(&[]),
            indent_start: 0.0,
            indent_hanging: 0.0,
            drop_lines: 0,
            drop_shift: 0.0,
        }
    }

    fn image_x(alignment: parley::Alignment) -> f32 {
        let mut layout = empty_layout();
        stack_block_images(&mut layout, &[one_inch_image()], 400.0, false, alignment);
        layout
            .items
            .iter()
            .find_map(|i| match i {
                PositionedItem::Image(img) => Some(img.rect.x()),
                _ => None,
            })
            .expect("an image item")
    }

    /// A block image follows its paragraph's `w:jc`.
    ///
    /// Every image used to be pinned to `x = 0`, which put `acid2-docx`'s
    /// centred chart against the left margin while Word centres it — the image
    /// then mismatched the golden *twice*, once where Word draws it and once
    /// where Loki did.
    ///
    /// A 72 pt image in a 400 pt column: centred leaves 164 pt each side, and
    /// end-aligned leaves 328 pt before it.
    #[test]
    fn a_block_image_follows_the_paragraph_alignment() {
        assert!(
            (image_x(parley::Alignment::Start) - 0.0).abs() < 0.01,
            "start-aligned must sit at the column's left edge"
        );
        assert!(
            (image_x(parley::Alignment::Center) - 164.0).abs() < 0.01,
            "centred must be ((400 - 72) / 2), got {}",
            image_x(parley::Alignment::Center)
        );
        assert!(
            (image_x(parley::Alignment::End) - 328.0).abs() < 0.01,
            "end-aligned must be (400 - 72), got {}",
            image_x(parley::Alignment::End)
        );
        // Justified is *not* centred: there is nothing to stretch a lone object
        // between, and Word leaves it at the start. `acid2-docx` has two
        // `w:jc="both"` image paragraphs that would move if this were wrong.
        assert!(
            (image_x(parley::Alignment::Justify) - 0.0).abs() < 0.01,
            "justified must behave as start, got {}",
            image_x(parley::Alignment::Justify)
        );
    }
}

#[cfg(test)]
mod fit_tests {
    use super::super::fit_to_column;
    use crate::mode::LayoutMode;

    /// An element already inside the column is untouched — fit-to-column is a
    /// ceiling, not a resize.
    #[test]
    fn an_element_that_fits_is_left_alone() {
        assert_eq!(fit_to_column(100.0, 50.0, 400.0), (100.0, 50.0));
        assert_eq!(fit_to_column(400.0, 50.0, 400.0), (400.0, 50.0));
    }

    /// **Oversized shrinks to the column with its aspect preserved.** The aspect
    /// is the assertion that matters: scaling only the width would squash the
    /// image, which is a worse defect than the sideways scroll being removed.
    #[test]
    fn an_oversized_element_shrinks_with_its_aspect_preserved() {
        let (w, h) = fit_to_column(800.0, 400.0, 400.0);
        assert_eq!(w, 400.0, "the width did not come down to the column");
        assert!(
            (h - 200.0).abs() < 1e-3,
            "height {h} does not preserve the 2:1 aspect — expected 200"
        );
        // A non-integer ratio, so the assertion above is not passing on a
        // halving that a width-only scale would also produce.
        let (w, h) = fit_to_column(1000.0, 300.0, 375.0);
        assert!((w - 375.0).abs() < 1e-3);
        assert!(
            (h - 112.5).abs() < 1e-3,
            "height {h} does not preserve the 10:3 aspect — expected 112.5"
        );
    }

    /// A degenerate input returns unchanged rather than dividing by zero or
    /// resizing on a measurement that has not arrived.
    #[test]
    fn degenerate_inputs_change_nothing() {
        assert_eq!(fit_to_column(0.0, 50.0, 400.0), (0.0, 50.0));
        assert_eq!(fit_to_column(800.0, 400.0, 0.0), (800.0, 400.0));
        assert_eq!(fit_to_column(800.0, 400.0, -1.0), (800.0, 400.0));
        // The inverse: a good input does shrink, so "unchanged" is not the only
        // answer this function knows.
        assert_ne!(fit_to_column(800.0, 400.0, 400.0), (800.0, 400.0));
    }

    /// **Only the reflow view fits to the column.** The paginated and pageless
    /// views are fidelity views of a page with a real width, where an oversized
    /// image overhangs exactly as Word and LibreOffice paint it.
    #[test]
    fn only_the_reflow_view_fits_oversized_elements() {
        assert!(
            LayoutMode::Reflow {
                available_width: 400.0
            }
            .fits_oversized_to_column()
        );
        assert!(
            !LayoutMode::Paginated.fits_oversized_to_column(),
            "the paginated view would silently shrink an oversized image"
        );
        assert!(!LayoutMode::Pageless.fits_oversized_to_column());
    }
}

#[cfg(test)]
mod line_box_tests {
    use super::super::stack_block_images;
    use crate::color::LayoutColor;
    use crate::geometry::{LayoutPoint, LayoutRect};
    use crate::items::{GlyphSynthesis, PositionedGlyphRun, PositionedItem, PositionedRect};
    use crate::para::ParagraphLayout;
    use crate::resolve::CollectedImage;

    /// An image `pt` points tall (EMU: 12700 per point).
    fn image(pt: f32) -> CollectedImage {
        CollectedImage {
            src: "data:image/png;base64,".into(),
            alt: None,
            cx_emu: 914_400,
            cy_emu: (pt * 12_700.0) as u64,
            float: None,
            textbox: None,
        }
    }

    fn layout_of_height(h: f32) -> ParagraphLayout {
        ParagraphLayout {
            height: h,
            width: 0.0,
            items: Vec::new(),
            first_baseline: 0.0,
            last_baseline: 0.0,
            line_boundaries: Vec::new(),
            parley_layout: None,
            orig_to_clean: crate::para::ByteIndexMap::from_indices(&[]),
            clean_to_orig: crate::para::ByteIndexMap::from_indices(&[]),
            indent_start: 0.0,
            indent_hanging: 0.0,
            drop_lines: 0,
            drop_shift: 0.0,
        }
    }

    /// A paragraph carrying a glyph run — i.e. one that actually has text.
    fn with_text(h: f32) -> ParagraphLayout {
        let mut l = layout_of_height(h);
        l.items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
            origin: LayoutPoint::new(0.0, 0.0),
            font_data: std::sync::Arc::new(Vec::new()),
            font_index: 0,
            font_size: 11.0,
            glyphs: Vec::new(),
            color: LayoutColor::BLACK,
            synthesis: GlyphSynthesis::default(),
            normalized_coords: Vec::new(),
            link_url: None,
        }));
        l
    }

    /// A paragraph whose only content is an image is as tall as the image, not
    /// the image *plus* an empty line.
    ///
    /// Word puts an inline image in the line box. Loki added the two, so
    /// `acid2-docx.docx`'s chart paragraph — a 108 pt drawing with `w:after="0"`
    /// and no text — measured 121.65 pt and pushed its "Figure 1" caption
    /// 13.65 pt below where Word puts it, carrying the rest of page 3 down with
    /// it. Page 3 went 87 → 59 failing regions when this was fixed.
    #[test]
    fn an_image_only_paragraph_is_as_tall_as_its_image() {
        let mut l = layout_of_height(13.65);
        stack_block_images(
            &mut l,
            &[image(108.0)],
            400.0,
            false,
            parley::Alignment::Left,
        );
        assert!(
            (l.height - 108.0).abs() < 0.01,
            "expected the image's 108pt, got {}",
            l.height
        );
    }

    /// The inversion: an image *shorter* than the line still leaves a
    /// line-height paragraph, so the rule is `max`, not "replace with the
    /// image". Without this, a small inline image would collapse its line.
    #[test]
    fn an_image_shorter_than_the_line_keeps_the_line_height() {
        let mut l = layout_of_height(13.65);
        stack_block_images(&mut l, &[image(4.0)], 400.0, false, parley::Alignment::Left);
        assert!(
            (l.height - 13.65).abs() < 0.01,
            "a 4pt image must not shrink a 13.65pt line, got {}",
            l.height
        );
    }

    /// The other inversion: with text present the images still **stack above
    /// it**, so the heights add. That is Loki's block-image model, and it is
    /// what keeps this fix from silently swallowing a caption's own line.
    #[test]
    fn images_still_stack_above_text_when_the_paragraph_has_any() {
        let mut l = with_text(13.65);
        stack_block_images(
            &mut l,
            &[image(108.0)],
            400.0,
            false,
            parley::Alignment::Left,
        );
        assert!(
            (l.height - 121.65).abs() < 0.01,
            "text + image must add (13.65 + 108), got {}",
            l.height
        );
    }

    /// Paragraph shading covers the image in both shapes — expanded when the
    /// image stacks above text, and at least the image's height when it does
    /// not.
    #[test]
    fn a_background_fill_covers_the_image_in_both_shapes() {
        for (mut l, want) in [(layout_of_height(13.65), 108.0), (with_text(13.65), 121.65)] {
            l.items.insert(
                0,
                PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(0.0, 0.0, 400.0, 13.65),
                    color: LayoutColor::BLACK,
                }),
            );
            stack_block_images(
                &mut l,
                &[image(108.0)],
                400.0,
                false,
                parley::Alignment::Left,
            );
            let bg = l
                .items
                .iter()
                .find_map(|i| match i {
                    PositionedItem::FilledRect(r) => Some(r.rect.size.height),
                    _ => None,
                })
                .expect("the background rect");
            assert!(
                (bg - want).abs() < 0.01,
                "background height {bg}, want {want}"
            );
        }
    }
}
