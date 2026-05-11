// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! DOCX export — write side.
//!
//! Converts a [`loki_doc_model::Document`] into a spec-conformant `.docx`
//! ZIP package via the `loki-opc` layer.
//!
//! Tier-3 fidelity per ADR-0007: all text, headings, lists, tables, and
//! section properties are serialized; images, footnotes, and complex fields
//! are dropped with silent omission.

pub(super) mod assembly;
mod document;
mod numbering;
mod styles;
mod xml;
