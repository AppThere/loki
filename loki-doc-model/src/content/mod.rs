// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document content model: blocks, inlines, tables, fields, and annotations.
//!
//! The content layer is inspired by `Text.Pandoc.Definition`'s `Block` and
//! `Inline` hierarchy, which has been validated against 30+ formats over
//! 18 years. Office-document-specific content types are added as extensions.
//! See ADR-0001.

pub mod annotation;
pub mod attr;
pub mod block;
pub mod field;
pub mod inline;
pub mod table;

pub use attr::{ExtensionBag, ExtensionKey, NodeAttr};
pub use block::Block;
pub use inline::{Inline, NoteKind};
