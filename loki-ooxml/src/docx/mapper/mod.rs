// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Translates intermediate OOXML model types → [`loki_doc_model`] types.
//!
//! This is the second step of the two-step DOCX import pipeline:
//!
//! 1. **XML → intermediate model** (`reader` layer)
//! 2. **Intermediate model → [`loki_doc_model`]** (this layer)
//!
//! Entry point: [`document::map_document`].

pub(crate) mod document;
pub(crate) mod images;
pub(crate) mod inline;
pub(crate) mod numbering;
pub(crate) mod paragraph;
pub(crate) mod props;
pub(crate) mod styles;
pub(crate) mod table;
