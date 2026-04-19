// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! DOCX (WordprocessingML) import for the Loki document suite.
//!
//! ECMA-376 Part 1, §17 — WordprocessingML.
//!
//! # Architecture
//!
//! Parsing proceeds in two steps:
//!
//! 1. **XML → intermediate model** ([`model`] + [`reader`]): `quick-xml`
//!    event-mode parsers populate crate-internal structs that mirror the raw
//!    OOXML XML hierarchy.
//!
//! 2. **Intermediate model → [`loki_doc_model`]** ([`mapper`]): the mapper
//!    layer translates the intermediate structs into the format-neutral
//!    abstract document model.

pub mod export;
pub mod import;
pub mod mapper;
pub(crate) mod model;
pub(crate) mod reader;
