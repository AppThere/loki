// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Slides and placeholder roles.

use loki_graphics::{Drawing, ShapeId, Size};
use serde::{Deserialize, Serialize};

/// A stable identifier for a slide within a presentation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlideId(String);

impl SlideId {
    /// Wraps a string as a slide id.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SlideId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The semantic role of a placeholder shape (which shape holds the title, body,
/// etc.). Presentation-specific; the underlying shape lives in the slide's
/// [`Drawing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaceholderKind {
    /// The slide title.
    Title,
    /// A title centered on a title slide.
    CenteredTitle,
    /// A subtitle (title slide).
    Subtitle,
    /// The main body / content area.
    Body,
    /// Notes text.
    Notes,
    /// A placeholder with no standard role.
    Other,
}

/// Associates a placeholder role with the shape that fills it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placeholder {
    /// The role this placeholder plays.
    pub kind: PlaceholderKind,
    /// The shape in the slide's drawing that occupies the placeholder.
    pub shape_id: ShapeId,
}

/// A single slide: a drawing page plus presentation semantics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    /// Stable identifier.
    pub id: SlideId,
    /// Optional author-visible name.
    pub name: Option<String>,
    /// The slide's shapes and background.
    pub drawing: Drawing,
    /// Placeholder roles mapping to shapes in `drawing`.
    pub placeholders: Vec<Placeholder>,
    /// Speaker notes, if any.
    pub notes: Option<String>,
}

impl Slide {
    /// An empty slide of the given size.
    pub fn new(id: impl Into<SlideId>, size: Size) -> Self {
        Self {
            id: id.into(),
            name: None,
            drawing: Drawing::new(size),
            placeholders: Vec::new(),
            notes: None,
        }
    }

    /// Records that `shape_id` fills the placeholder `kind`.
    pub fn add_placeholder(&mut self, kind: PlaceholderKind, shape_id: impl Into<ShapeId>) {
        self.placeholders.push(Placeholder {
            kind,
            shape_id: shape_id.into(),
        });
    }

    /// Returns the shape id filling the given placeholder role, if any.
    pub fn placeholder(&self, kind: PlaceholderKind) -> Option<&ShapeId> {
        self.placeholders
            .iter()
            .find(|p| p.kind == kind)
            .map(|p| &p.shape_id)
    }
}

impl From<&str> for SlideId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for SlideId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_graphics::{RectF, Shape, TextBody};

    #[test]
    fn placeholder_lookup() {
        let size = Size::new(960.0, 540.0);
        let mut slide = Slide::new("s1", size);
        slide.drawing.push(Shape::text_box(
            "title",
            RectF::new(40.0, 40.0, 880.0, 100.0),
            TextBody::plain("Hello"),
        ));
        slide.add_placeholder(PlaceholderKind::Title, "title");

        assert_eq!(slide.id.as_str(), "s1");
        assert_eq!(
            slide
                .placeholder(PlaceholderKind::Title)
                .map(|i| i.as_str()),
            Some("title")
        );
        assert!(slide.placeholder(PlaceholderKind::Body).is_none());
    }
}
