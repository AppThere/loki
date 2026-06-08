// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Intermediate Rust structs mirroring the raw OOXML XML structure.
//!
//! These types are the deserialization targets for `quick-xml` event parsing.
//! They closely mirror the XML element hierarchy defined in ECMA-376. They are
//! **crate-internal** — never exposed in the public API. The mapper layer
//! translates these into [`loki_doc_model`] types.

pub mod document;
pub mod fields;
pub mod footnotes;
pub mod numbering;
pub mod paragraph;
pub mod section;
pub mod settings;
pub mod styles;
