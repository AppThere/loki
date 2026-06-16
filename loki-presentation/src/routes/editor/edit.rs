// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Pure model mutations for the presentation editor.
//!
//! These operate directly on the [`Presentation`] held in the editor's signal.
//! Setting text collapses the target paragraph to a single run, preserving its
//! character props — adequate for the current plain-text editing surface.

use loki_graphics::{RectF, Shape, ShapeId, ShapeKind, Size, TextBody, TextParagraph, TextRun};
use loki_presentation_model::{PlaceholderKind, Presentation, Slide};

/// Sets paragraph `para` of the shape `shape_id` on `slide` to `text`.
///
/// Creates missing paragraphs/runs as needed; preserves the first run's props.
/// No-op if the slide or shape is missing or the shape has no text body.
pub(super) fn set_shape_text(
    pres: &mut Presentation,
    slide: usize,
    shape_id: &ShapeId,
    para: usize,
    text: &str,
) {
    let Some(slide) = pres.slides.get_mut(slide) else {
        return;
    };
    let Some(shape) = slide.drawing.shape_mut(shape_id) else {
        return;
    };
    let ShapeKind::Geometry(g) = &mut shape.kind else {
        return;
    };
    let body = g.text.get_or_insert_with(TextBody::default);
    while body.paragraphs.len() <= para {
        body.paragraphs.push(TextParagraph::default());
    }
    let p = &mut body.paragraphs[para];
    if p.runs.is_empty() {
        p.runs.push(TextRun::default());
    }
    p.runs[0].text = text.to_string();
    p.runs.truncate(1);
}

/// Appends a new editable slide (with empty title + body placeholders).
pub(super) fn add_slide(pres: &mut Presentation) {
    let max = pres
        .slides
        .iter()
        .filter_map(|s| s.id.as_str().strip_prefix("slide")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    let id = format!("slide{}", max + 1);
    let slide = new_editable_slide(&id, pres.slide_size);
    pres.add_slide(slide);
}

/// Removes the slide at `index`, keeping at least one slide.
pub(super) fn delete_slide(pres: &mut Presentation, index: usize) {
    if pres.slides.len() > 1 && index < pres.slides.len() {
        pres.slides.remove(index);
    }
}

/// Appends an empty bullet paragraph to the slide's body placeholder, creating
/// the body placeholder if the slide doesn't have one.
pub(super) fn add_bullet(pres: &mut Presentation, slide: usize) {
    let size = pres.slide_size;
    let Some(s) = pres.slides.get_mut(slide) else {
        return;
    };
    let body_id = match s.placeholder(PlaceholderKind::Body).cloned() {
        Some(id) => id,
        None => {
            let id = ShapeId::new("body");
            s.drawing.push(Shape::text_box(
                id.clone(),
                body_frame(size),
                TextBody::default(),
            ));
            s.add_placeholder(PlaceholderKind::Body, id.clone());
            id
        }
    };
    if let Some(shape) = s.drawing.shape_mut(&body_id)
        && let ShapeKind::Geometry(g) = &mut shape.kind
    {
        g.text
            .get_or_insert_with(TextBody::default)
            .paragraphs
            .push(TextParagraph::default());
    }
}

fn new_editable_slide(id: &str, size: Size) -> Slide {
    let mut s = Slide::new(id, size);
    s.drawing.push(Shape::text_box(
        "title",
        title_frame(size),
        TextBody::default(),
    ));
    s.add_placeholder(PlaceholderKind::Title, "title");
    s.drawing.push(Shape::text_box(
        "body",
        body_frame(size),
        TextBody::default(),
    ));
    s.add_placeholder(PlaceholderKind::Body, "body");
    s
}

fn title_frame(size: Size) -> RectF {
    RectF::new(40.0, 30.0, (size.width - 80.0).max(0.0), 90.0)
}

fn body_frame(size: Size) -> RectF {
    RectF::new(
        40.0,
        140.0,
        (size.width - 80.0).max(0.0),
        (size.height - 180.0).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_text_creates_paragraphs_and_run() {
        let mut p = Presentation::default();
        add_slide(&mut p);
        let title_id = ShapeId::new("title");
        set_shape_text(&mut p, 0, &title_id, 0, "Hello");
        let v = super::super::slide_view::slide_views(&p);
        assert_eq!(v[0].title.as_ref().unwrap().text, "Hello");
    }

    #[test]
    fn add_slide_is_editable() {
        let mut p = Presentation::default();
        assert_eq!(p.slide_count(), 0);
        add_slide(&mut p);
        assert_eq!(p.slide_count(), 1);
        // Title + body placeholders exist.
        assert!(p.slides[0].placeholder(PlaceholderKind::Title).is_some());
        assert!(p.slides[0].placeholder(PlaceholderKind::Body).is_some());
    }

    #[test]
    fn delete_keeps_at_least_one_slide() {
        let mut p = Presentation::default();
        add_slide(&mut p);
        delete_slide(&mut p, 0);
        assert_eq!(p.slide_count(), 1); // not removed — last slide
        add_slide(&mut p);
        delete_slide(&mut p, 0);
        assert_eq!(p.slide_count(), 1);
    }

    #[test]
    fn add_bullet_appends_paragraph() {
        let mut p = Presentation::default();
        add_slide(&mut p);
        add_bullet(&mut p, 0);
        add_bullet(&mut p, 0);
        let body_id = ShapeId::new("body");
        set_shape_text(&mut p, 0, &body_id, 1, "second");
        let v = super::super::slide_view::slide_views(&p);
        assert!(v[0].bullets.iter().any(|b| b.text == "second"));
    }
}
