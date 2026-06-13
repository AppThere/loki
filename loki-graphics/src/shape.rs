// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shapes: positioned, transformed graphic objects.
//!
//! A [`Shape`] pairs a [`ShapeTransform`] (where it sits on the page) with a
//! [`ShapeKind`] (what it is). Geometry is defined in the frame's local space
//! and placed by the transform — the OOXML/ODF "geometry in a box" model. A
//! text box is simply a [`GeometryShape`] with a rectangle preset and a
//! [`TextBody`]; a group nests child shapes; an image references raster data.

use crate::geometry::RectF;
use crate::id::ShapeId;
use crate::path::Path;
use crate::preset::PresetShape;
use crate::style::{Fill, Stroke};
use crate::text::TextBody;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Where a shape sits on the page and how it is oriented.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ShapeTransform {
    /// Bounding box of the shape on the page, in points.
    pub frame: RectF,
    /// Clockwise rotation about the frame center, in degrees.
    pub rotation_deg: f64,
    /// Mirror horizontally.
    pub flip_h: bool,
    /// Mirror vertically.
    pub flip_v: bool,
}

impl ShapeTransform {
    /// An unrotated, unflipped transform for `frame`.
    pub fn new(frame: RectF) -> Self {
        Self {
            frame,
            rotation_deg: 0.0,
            flip_h: false,
            flip_v: false,
        }
    }
}

/// The outline of a [`GeometryShape`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Geometry {
    /// A built-in preset outline.
    Preset(PresetShape),
    /// A custom outline.
    Custom(Path),
}

/// A drawn shape: an outline with fill, stroke, and optional text.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GeometryShape {
    /// The outline.
    pub geometry: Geometry,
    /// Interior fill.
    pub fill: Fill,
    /// Outline stroke, if any.
    pub stroke: Option<Stroke>,
    /// Text drawn inside the shape, if any.
    pub text: Option<TextBody>,
}

/// A group of child shapes positioned within this shape's frame.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Group {
    /// Child shapes.
    pub children: Vec<Shape>,
}

/// Raster image data referenced by an [`ImageShape`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageRef {
    /// Encoded image format.
    pub format: ImageFormat,
    /// Where the bytes come from.
    pub source: ImageSource,
}

/// Encoded raster format.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// GIF.
    Gif,
    /// SVG (vector, but referenced as opaque image data here).
    Svg,
    /// Another format identified by media type.
    Other(String),
}

/// Where an [`ImageRef`]'s bytes live.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ImageSource {
    /// Bytes embedded directly in the model.
    Embedded(Vec<u8>),
    /// An external/package-relative identifier resolved by the host.
    External(String),
}

/// An image placed in a frame.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImageShape {
    /// The referenced image data.
    pub image: ImageRef,
}

/// What a [`Shape`] is.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ShapeKind {
    /// A drawn outline (incl. text boxes).
    Geometry(GeometryShape),
    /// A nested group of shapes.
    Group(Group),
    /// A raster image.
    Image(ImageShape),
}

/// A positioned graphic object.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Shape {
    /// Stable identifier.
    pub id: ShapeId,
    /// Optional author-visible name.
    pub name: Option<String>,
    /// Placement and orientation.
    pub transform: ShapeTransform,
    /// The shape's content.
    pub kind: ShapeKind,
}

impl Shape {
    /// A geometry shape (preset or custom) with the given fill/stroke and no text.
    pub fn geometry(
        id: impl Into<ShapeId>,
        frame: RectF,
        geometry: Geometry,
        fill: Fill,
        stroke: Option<Stroke>,
    ) -> Self {
        Self {
            id: id.into(),
            name: None,
            transform: ShapeTransform::new(frame),
            kind: ShapeKind::Geometry(GeometryShape {
                geometry,
                fill,
                stroke,
                text: None,
            }),
        }
    }

    /// A rectangular text box (no fill, no stroke) carrying `text`.
    pub fn text_box(id: impl Into<ShapeId>, frame: RectF, text: TextBody) -> Self {
        Self {
            id: id.into(),
            name: None,
            transform: ShapeTransform::new(frame),
            kind: ShapeKind::Geometry(GeometryShape {
                geometry: Geometry::Preset(PresetShape::Rectangle),
                fill: Fill::None,
                stroke: None,
                text: Some(text),
            }),
        }
    }

    /// A group of child shapes occupying `frame`.
    pub fn group(id: impl Into<ShapeId>, frame: RectF, children: Vec<Shape>) -> Self {
        Self {
            id: id.into(),
            name: None,
            transform: ShapeTransform::new(frame),
            kind: ShapeKind::Group(Group { children }),
        }
    }

    /// Sets the author-visible name (builder style).
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::RectF;

    fn frame() -> RectF {
        RectF::new(0.0, 0.0, 100.0, 50.0)
    }

    #[test]
    fn geometry_shape_constructor() {
        let s = Shape::geometry(
            "s1",
            frame(),
            Geometry::Preset(PresetShape::Ellipse),
            Fill::None,
            None,
        );
        assert_eq!(s.id.as_str(), "s1");
        assert_eq!(s.transform.rotation_deg, 0.0);
        matches!(s.kind, ShapeKind::Geometry(_));
    }

    #[test]
    fn text_box_has_text_and_rect_geometry() {
        let s = Shape::text_box("t", frame(), TextBody::plain("hi")).with_name("Title");
        assert_eq!(s.name.as_deref(), Some("Title"));
        if let ShapeKind::Geometry(g) = &s.kind {
            assert!(matches!(
                g.geometry,
                Geometry::Preset(PresetShape::Rectangle)
            ));
            assert_eq!(g.text.as_ref().unwrap().paragraphs[0].text(), "hi");
        } else {
            panic!("expected geometry kind");
        }
    }

    #[test]
    fn group_nests_children() {
        let child = Shape::text_box("c", frame(), TextBody::plain("x"));
        let g = Shape::group("g", frame(), vec![child]);
        if let ShapeKind::Group(grp) = &g.kind {
            assert_eq!(grp.children.len(), 1);
        } else {
            panic!("expected group kind");
        }
    }
}
