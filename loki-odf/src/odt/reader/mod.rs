// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! XML readers for ODF document parts.
//!
//! Each reader accepts raw XML bytes (`&[u8]`) and returns the corresponding
//! intermediate model type.  All readers set `trim_text(false)` so that
//! significant whitespace inside `text:span` and similar elements is
//! preserved verbatim.

pub(crate) mod document;
pub(crate) mod meta;
pub(crate) mod styles;
