// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Markdown import (usage audit §12): CommonMark plus the GFM extensions the
//! model can express (tables, strikethrough, task lists as text) into the
//! suite's [`Document`].
//!
//! Structure maps onto the markdown template's style ids — `Heading1..6`,
//! `Blockquote`, `CodeBlock`, `Normal` — and lists map onto the modern
//! `StyledPara` + `list_id` representation using the built-in default list
//! styles (seeded into the document's catalog), so an imported list is
//! editable, exportable, and Tab-promotable like a native one.
//!
//! **The importer emits style references only** (plus the two list-style
//! definitions it owns); the paragraph catalog that gives them geometry is
//! the caller's to merge — the editor merges the bundled markdown template's
//! catalog. Import-only; Markdown export is a non-goal here.

#![forbid(unsafe_code)]

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;

mod convert;

/// Errors from Markdown import.
#[derive(Debug, thiserror::Error)]
pub enum MarkdownError {
    /// The reader failed.
    #[error("I/O error reading Markdown input: {0}")]
    Io(#[from] std::io::Error),
    /// The input was not UTF-8 text.
    #[error("Markdown input is not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

/// The paragraph style ids the emitted document references — the caller's
/// catalog must define them (the bundled markdown template does).
pub const REQUIRED_STYLE_IDS: &[&str] = &[
    "Normal",
    "Heading1",
    "Heading2",
    "Heading3",
    "Heading4",
    "Heading5",
    "Heading6",
    "Blockquote",
    "CodeBlock",
];

/// Markdown importer (see the crate docs).
pub struct MarkdownImport;

impl DocumentImport for MarkdownImport {
    type Error = MarkdownError;
    type Options = ();

    fn import(
        mut reader: impl std::io::Read + std::io::Seek,
        (): Self::Options,
    ) -> Result<Document, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        let text = String::from_utf8(bytes)?;
        Ok(parse_str(&text))
    }
}

/// Parses Markdown source into a [`Document`].
#[must_use]
pub fn parse_str(text: &str) -> Document {
    convert::convert(text)
}
