// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Derives a renderable, read-only view of each slide from the presentation
//! model.
//!
//! Blitz (the HTML/CSS renderer) does not support absolute positioning, so the
//! editor cannot yet place shapes at their exact slide coordinates. Until the
//! GPU slide canvas lands, this maps each slide's placeholder/text shapes to a
//! readable title / subtitle / bullet flow that reflects the real imported
//! content.

use loki_graphics::{Fill, Shape, ShapeKind, TextBody};
use loki_presentation_model::{PlaceholderKind, Presentation, Slide};

/// A flattened, render-ready view of one slide.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SlideView {
    /// Title text (from the title placeholder), if any.
    pub title: String,
    /// Subtitle text (from the subtitle placeholder), if any.
    pub subtitle: String,
    /// Body lines (body placeholder paragraphs, or other text shapes).
    pub bullets: Vec<String>,
    /// Background color as a CSS hex string.
    pub bg_css: String,
    /// Foreground/text color as a CSS hex string.
    pub fg_css: String,
}

/// Maps every slide in `pres` to a [`SlideView`].
pub(super) fn slide_views(pres: &Presentation) -> Vec<SlideView> {
    pres.slides.iter().map(slide_to_view).collect()
}

fn slide_to_view(slide: &Slide) -> SlideView {
    let title = placeholder_text(slide, PlaceholderKind::Title)
        .or_else(|| placeholder_text(slide, PlaceholderKind::CenteredTitle))
        .unwrap_or_default();
    let subtitle = placeholder_text(slide, PlaceholderKind::Subtitle).unwrap_or_default();

    let title_id = slide
        .placeholder(PlaceholderKind::Title)
        .or_else(|| slide.placeholder(PlaceholderKind::CenteredTitle));
    let subtitle_id = slide.placeholder(PlaceholderKind::Subtitle);

    let bullets = if let Some(body_id) = slide.placeholder(PlaceholderKind::Body) {
        shape_paragraphs(slide.drawing.shape(body_id))
    } else {
        // No body placeholder: gather every text shape that isn't the title or
        // subtitle, one bullet per non-empty paragraph.
        slide
            .drawing
            .shapes
            .iter()
            .filter(|s| Some(&s.id) != title_id && Some(&s.id) != subtitle_id)
            .flat_map(|s| shape_paragraphs(Some(s)))
            .collect()
    };

    SlideView {
        title,
        subtitle,
        bullets,
        bg_css: fill_css(&slide.drawing.background).unwrap_or_else(|| "#FFFFFF".to_string()),
        fg_css: first_text_color(slide).unwrap_or_else(|| "#1A1A1A".to_string()),
    }
}

fn shape_textbody(shape: &Shape) -> Option<&TextBody> {
    match &shape.kind {
        ShapeKind::Geometry(g) => g.text.as_ref(),
        _ => None,
    }
}

fn placeholder_text(slide: &Slide, kind: PlaceholderKind) -> Option<String> {
    let id = slide.placeholder(kind)?;
    let body = shape_textbody(slide.drawing.shape(id)?)?;
    let joined = body
        .paragraphs
        .iter()
        .map(loki_graphics::TextParagraph::text)
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = joined.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn shape_paragraphs(shape: Option<&Shape>) -> Vec<String> {
    shape
        .and_then(shape_textbody)
        .map(|b| {
            b.paragraphs
                .iter()
                .map(loki_graphics::TextParagraph::text)
                .filter(|t| !t.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn fill_css(fill: &Fill) -> Option<String> {
    match fill {
        Fill::Solid(color) => color.to_hex(),
        _ => None,
    }
}

fn first_text_color(slide: &Slide) -> Option<String> {
    for shape in &slide.drawing.shapes {
        let Some(body) = shape_textbody(shape) else {
            continue;
        };
        for para in &body.paragraphs {
            for run in &para.runs {
                if let Some(color) = &run.props.color
                    && let Some(hex) = color.to_hex()
                {
                    return Some(hex);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_graphics::{Fill, RectF, Shape, TextBody};
    use loki_presentation_model::Slide;

    fn frame() -> RectF {
        RectF::new(0.0, 0.0, 100.0, 50.0)
    }

    #[test]
    fn maps_title_subtitle_and_body() {
        let mut p = Presentation::default();
        let mut slide = Slide::new("s1", p.slide_size);
        slide
            .drawing
            .push(Shape::text_box("t", frame(), TextBody::plain("My Title")));
        slide.drawing.push(Shape::text_box(
            "sub",
            frame(),
            TextBody::plain("A subtitle"),
        ));
        // Body with two paragraphs.
        let body = TextBody {
            paragraphs: vec![
                loki_graphics::TextParagraph::plain("First point"),
                loki_graphics::TextParagraph::plain("Second point"),
            ],
            ..Default::default()
        };
        slide.drawing.push(Shape::text_box("b", frame(), body));
        slide.add_placeholder(PlaceholderKind::Title, "t");
        slide.add_placeholder(PlaceholderKind::Subtitle, "sub");
        slide.add_placeholder(PlaceholderKind::Body, "b");
        p.add_slide(slide);

        let views = slide_views(&p);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].title, "My Title");
        assert_eq!(views[0].subtitle, "A subtitle");
        assert_eq!(views[0].bullets, vec!["First point", "Second point"]);
    }

    #[test]
    fn no_placeholders_collects_text_shapes_as_bullets() {
        let mut p = Presentation::default();
        let mut slide = Slide::new("s1", p.slide_size);
        slide
            .drawing
            .push(Shape::text_box("a", frame(), TextBody::plain("Loose text")));
        p.add_slide(slide);

        let views = slide_views(&p);
        assert_eq!(views[0].title, "");
        assert_eq!(views[0].bullets, vec!["Loose text"]);
    }

    #[test]
    fn solid_background_becomes_css_hex() {
        use loki_primitives::color::DocumentColor;
        let mut p = Presentation::default();
        let mut slide = Slide::new("s1", p.slide_size);
        slide.drawing.background = Fill::Solid(DocumentColor::from_hex("#1E1E1E").unwrap());
        p.add_slide(slide);

        let views = slide_views(&p);
        assert_eq!(views[0].bg_css, "#1E1E1E");
    }

    #[test]
    fn empty_presentation_yields_no_views() {
        let p = Presentation::default();
        assert!(slide_views(&p).is_empty());
    }
}
