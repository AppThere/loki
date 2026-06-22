// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX export — write side.
//!
//! Converts a [`loki_doc_model::Document`] into a spec-conformant `.docx`
//! ZIP package via the `loki-opc` layer.
//!
//! Tier-3 fidelity per ADR-0007: all text, headings, lists, tables, and
//! section properties are serialized; images, footnotes, and complex fields
//! are dropped with silent omission.

pub(super) mod assembly;
pub(super) mod collector;
mod document;
mod fields;
pub(super) mod footnotes;
pub(super) mod media;
mod numbering;
mod rels;
mod section;
mod settings;
mod style_props;
mod styles;
mod xml;
