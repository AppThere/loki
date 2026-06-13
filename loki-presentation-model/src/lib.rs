// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Format-neutral presentation document model for the Loki suite.
//!
//! A presentation is a deck of [`Slide`]s, each wrapping a
//! [`loki_graphics::Drawing`] (its shapes and background) plus presentation
//! semantics — placeholder roles and speaker notes. This mirrors how ODF
//! presentations (ODP) extend ODF drawings (ODG) and how OOXML PresentationML
//! builds on DrawingML: the reusable vector layer lives in `loki-graphics`
//! (shared with the planned Iris Draw), and this crate adds the presentation
//! layer on top.
//!
//! [`loro_bridge`] maps the model to/from a Loro CRDT for persistence, undo,
//! and (later) collaboration.

#![forbid(unsafe_code)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

pub mod loro_bridge;
pub mod presentation;
pub mod slide;

pub use loro_bridge::{BridgeError, loro_to_presentation, presentation_to_loro};
pub use presentation::{Presentation, PresentationMeta};
pub use slide::{Placeholder, PlaceholderKind, Slide, SlideId};
