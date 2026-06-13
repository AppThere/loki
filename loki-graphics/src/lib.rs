// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Format-neutral vector-graphics model for the AppThere suite.
//!
//! `loki-graphics` is the shared, renderer-agnostic model for 2D vector
//! drawings: shapes (preset or custom-path geometry), fills, strokes, gradients,
//! shape text, groups, and images, arranged on a fixed-size [`Drawing`] page.
//! It depends only on `loki-primitives` (for theme-aware [`DocumentColor`]), not
//! on any CRDT or renderer, so it is reused by:
//!
//! - **Loki Presentation** (`loki-presentation-model`), where a slide wraps a
//!   [`Drawing`] — mirroring how ODF presentations (ODP) extend ODF drawings
//!   (ODG) and how OOXML PresentationML builds on DrawingML; and
//! - **Iris Draw** (planned), which edits a [`Drawing`] directly.
//!
//! All coordinates are in points (1/72 inch). See [`geometry`] for the
//! coordinate conventions.
//!
//! [`DocumentColor`]: loki_primitives::color::DocumentColor

#![forbid(unsafe_code)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

pub mod drawing;
pub mod geometry;
pub mod id;
pub mod path;
pub mod preset;
pub mod shape;
pub mod style;
pub mod text;

pub use drawing::Drawing;
pub use geometry::{RectF, Size, Vec2};
pub use id::ShapeId;
pub use path::{Path, PathSegment};
pub use preset::PresetShape;
pub use shape::{
    Geometry, GeometryShape, Group, ImageFormat, ImageRef, ImageShape, ImageSource, Shape,
    ShapeKind, ShapeTransform,
};
pub use style::{Fill, GradientStop, LineCap, LineDash, LineJoin, LinearGradient, Stroke};
pub use text::{TextAlign, TextBody, TextParagraph, TextRun, TextRunProps, VerticalAnchor};
