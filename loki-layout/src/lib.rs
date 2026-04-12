// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Renderer-agnostic layout engine for the Loki suite.
//!
//! `loki-layout` takes a [`loki_doc_model::Document`] and produces a layout
//! result containing absolute positions for all content elements. It has no
//! GPU dependencies and is fully testable without a display.
//!
//! # Layout Modes
//!
//! Three modes are supported via [`LayoutMode`]:
//!
//! - [`LayoutMode::Paginated`]: content broken into fixed-size pages.
//! - [`LayoutMode::Pageless`]: single infinite canvas, document-width content.
//! - [`LayoutMode::Reflow`]: single infinite canvas, caller-supplied width.
//!
//! # Output
//!
//! Layout produces a [`DocumentLayout`] containing [`PositionedItem`]s, each
//! carrying absolute coordinates ready for a renderer such as `loki-vello`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod color;
pub mod error;
pub mod font;
pub mod geometry;
pub mod items;
pub mod mode;
pub mod para;
pub mod result;

pub use color::LayoutColor;
pub use error::{LayoutError, LayoutResult};
pub use geometry::{LayoutInsets, LayoutPoint, LayoutRect, LayoutSize};
pub use items::{
    BorderEdge, BorderStyle, DecorationKind, GlyphEntry, GlyphSynthesis,
    PositionedBorderRect, PositionedDecoration, PositionedGlyphRun, PositionedImage,
    PositionedItem, PositionedRect,
};
pub use font::FontResources;
pub use mode::LayoutMode;
pub use para::{layout_paragraph, ParagraphLayout, ResolvedParaProps, StyleSpan};
pub use result::{ContinuousLayout, DocumentLayout, LayoutPage, PaginatedLayout};
