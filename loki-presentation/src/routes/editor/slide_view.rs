// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Derives a renderable, **edit-aware** view of each slide from the model.
//!
//! Blitz (HTML/CSS) has no absolute positioning, so the editor cannot place
//! shapes at their exact slide coordinates yet. This flattens each slide to a
//! title / subtitle / bullet flow, carrying the originating shape id (and
//! paragraph index) with each field so edits write back to exactly the shape
//! the value was read from. Faithful per-shape placement is the GPU-canvas
//! follow-up.

use loki_graphics::{Fill, Shape, ShapeId, ShapeKind, TextBody};
use loki_presentation_model::{PlaceholderKind, Presentation, Slide};

/// An editable single-line field bound to paragraph 0 of a shape.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Editable {
    /// The shape backing this field.
    pub shape_id: ShapeId,
    /// Current text (paragraph 0).
    pub text: String,
}

/// An editable bullet line bound to a specific paragraph of a shape.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct EditableLine {
    /// The shape backing this line.
    pub shape_id: ShapeId,
    /// Paragraph index within the shape's text body.
    pub para: usize,
    /// Current text.
    pub text: String,
}

/// A flattened, edit-aware view of one slide.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SlideView {
    /// The slide's stable identifier — the Dioxus list key for thumbnails, so
    /// inserting/deleting slides re-uses the right DOM nodes (F7b).
    pub slide_id: String,
    /// Title field, if the slide has a title placeholder.
    pub title: Option<Editable>,
    /// Subtitle field, if present.
    pub subtitle: Option<Editable>,
    /// Body lines (body placeholder paragraphs, else other text shapes).
    pub bullets: Vec<EditableLine>,
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
    let title = editable_for(slide, PlaceholderKind::Title)
        .or_else(|| editable_for(slide, PlaceholderKind::CenteredTitle));
    let subtitle = editable_for(slide, PlaceholderKind::Subtitle);

    let title_id = slide
        .placeholder(PlaceholderKind::Title)
        .or_else(|| slide.placeholder(PlaceholderKind::CenteredTitle));
    let subtitle_id = slide.placeholder(PlaceholderKind::Subtitle);

    let bullets = if let Some(body_id) = slide.placeholder(PlaceholderKind::Body) {
        lines_of(slide, body_id)
    } else {
        slide
            .drawing
            .shapes
            .iter()
            .filter(|s| Some(&s.id) != title_id && Some(&s.id) != subtitle_id)
            .flat_map(|s| lines_of(slide, &s.id))
            .collect()
    };

    SlideView {
        slide_id: slide.id.as_str().to_string(),
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

/// An [`Editable`] for the placeholder of `kind`, if its shape exists (even when
/// the text is empty — an empty placeholder is still editable).
fn editable_for(slide: &Slide, kind: PlaceholderKind) -> Option<Editable> {
    let id = slide.placeholder(kind)?;
    // The shape must exist to be editable.
    let shape = slide.drawing.shape(id)?;
    let text = shape_textbody(shape)
        .and_then(|b| b.paragraphs.first())
        .map(loki_graphics::TextParagraph::text)
        .unwrap_or_default();
    Some(Editable {
        shape_id: id.clone(),
        text,
    })
}

/// One [`EditableLine`] per paragraph of the shape with `id`.
fn lines_of(slide: &Slide, id: &ShapeId) -> Vec<EditableLine> {
    let Some(shape) = slide.drawing.shape(id) else {
        return Vec::new();
    };
    let Some(body) = shape_textbody(shape) else {
        return Vec::new();
    };
    body.paragraphs
        .iter()
        .enumerate()
        .map(|(para, p)| EditableLine {
            shape_id: id.clone(),
            para,
            text: p.text(),
        })
        .collect()
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
    use loki_graphics::{RectF, Shape, TextBody, TextParagraph};
    use loki_presentation_model::Slide;

    fn frame() -> RectF {
        RectF::new(0.0, 0.0, 100.0, 50.0)
    }

    #[test]
    fn title_and_bullets_carry_shape_ids() {
        let mut p = Presentation::default();
        let mut slide = Slide::new("s1", p.slide_size);
        slide
            .drawing
            .push(Shape::text_box("t", frame(), TextBody::plain("My Title")));
        let body = TextBody {
            paragraphs: vec![TextParagraph::plain("One"), TextParagraph::plain("Two")],
            ..Default::default()
        };
        slide.drawing.push(Shape::text_box("b", frame(), body));
        slide.add_placeholder(PlaceholderKind::Title, "t");
        slide.add_placeholder(PlaceholderKind::Body, "b");
        p.add_slide(slide);

        let v = &slide_views(&p)[0];
        let title = v.title.as_ref().unwrap();
        assert_eq!(title.shape_id.as_str(), "t");
        assert_eq!(title.text, "My Title");
        assert_eq!(v.bullets.len(), 2);
        assert_eq!(v.bullets[1].shape_id.as_str(), "b");
        assert_eq!(v.bullets[1].para, 1);
        assert_eq!(v.bullets[1].text, "Two");
    }

    #[test]
    fn empty_title_placeholder_is_still_editable() {
        let mut p = Presentation::default();
        let mut slide = Slide::new("s1", p.slide_size);
        slide
            .drawing
            .push(Shape::text_box("t", frame(), TextBody::default()));
        slide.add_placeholder(PlaceholderKind::Title, "t");
        p.add_slide(slide);

        let v = &slide_views(&p)[0];
        let title = v.title.as_ref().unwrap();
        assert_eq!(title.shape_id.as_str(), "t");
        assert_eq!(title.text, "");
    }

    #[test]
    fn slide_without_placeholders_has_no_title() {
        let mut p = Presentation::default();
        let mut slide = Slide::new("s1", p.slide_size);
        slide
            .drawing
            .push(Shape::text_box("x", frame(), TextBody::plain("Loose")));
        p.add_slide(slide);

        let v = &slide_views(&p)[0];
        assert!(v.title.is_none());
        assert_eq!(v.bullets[0].text, "Loose");
    }
}
