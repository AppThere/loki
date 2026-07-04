// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! Compatibility shims for non-conformant OPC packages produced by real-world
//! OOXML writers ([MS-OI29500] / [MS-OE376]).
//!
//! Each shim is a no-op under the `strict` feature and otherwise repairs a
//! specific deviation — backslash separators ([`zip_names`]), non-canonical
//! percent encoding and duplicate part names ([`part_names`]), a misplaced
//! content-types part ([`content_types`]), duplicate relationship ids
//! ([`relationships`]) — recording a [`crate::error::DeviationWarning`] for
//! each repair rather than rejecting the package.

pub mod content_types;
pub mod part_names;
pub mod relationships;
pub mod zip_names;
