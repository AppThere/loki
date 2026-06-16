// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A drawing page: the reusable canvas unit.
//!
//! A [`Drawing`] is a fixed-size page (in points) with a background fill and an
//! ordered list of [`Shape`]s painted back-to-front. This is the ODG-page
//! equivalent and the unit Iris Draw edits directly; a presentation slide wraps
//! one (see `loki-presentation-model`).

use crate::geometry::Size;
use crate::id::ShapeId;
use crate::shape::Shape;
use crate::style::Fill;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A fixed-size canvas of shapes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Drawing {
    /// Page size in points.
    pub size: Size,
    /// Page background fill.
    pub background: Fill,
    /// Shapes in back-to-front paint order.
    pub shapes: Vec<Shape>,
}

impl Drawing {
    /// An empty drawing of the given size with no background.
    pub fn new(size: Size) -> Self {
        Self {
            size,
            background: Fill::None,
            shapes: Vec::new(),
        }
    }

    /// Appends a shape (drawn on top of existing shapes).
    pub fn push(&mut self, shape: Shape) {
        self.shapes.push(shape);
    }

    /// Finds a top-level shape by id (does not descend into groups).
    pub fn shape(&self, id: &ShapeId) -> Option<&Shape> {
        self.shapes.iter().find(|s| &s.id == id)
    }

    /// Finds a top-level shape by id for mutation (does not descend into groups).
    pub fn shape_mut(&mut self, id: &ShapeId) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|s| &s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::RectF;
    use crate::text::TextBody;

    #[test]
    fn push_and_lookup() {
        let mut d = Drawing::new(Size::new(960.0, 540.0));
        assert!(d.shapes.is_empty());
        d.push(Shape::text_box(
            "t1",
            RectF::new(0.0, 0.0, 100.0, 40.0),
            TextBody::plain("hi"),
        ));
        assert_eq!(d.shapes.len(), 1);
        assert!(d.shape(&ShapeId::new("t1")).is_some());
        assert!(d.shape(&ShapeId::new("nope")).is_none());
    }
}
