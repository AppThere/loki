// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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
pub mod float;
pub mod inline;
pub mod table;
pub mod toc;

pub use attr::{ExtensionBag, ExtensionKey, NodeAttr};
pub use block::Block;
pub use float::{FloatWrap, TextWrap, WrapSide};
pub use inline::{Inline, NoteKind};
