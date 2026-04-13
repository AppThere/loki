// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Mappers from ODF intermediate model types to the format-neutral
//! `loki_doc_model` types.
//!
//! Each sub-module handles one layer of the conversion:
//!
//! - [`props`] — paragraph and character property conversion
//! - [`styles`] — stylesheet → [`StyleCatalog`] mapping
//! - [`lists`] — list-style → [`ListStyle`] mapping
//! - [`document`] — full document → [`loki_doc_model::Document`] mapping

pub(crate) mod document;
pub(crate) mod lists;
pub(crate) mod props;
pub(crate) mod styles;
