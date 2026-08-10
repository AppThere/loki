// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Fountain screenplay import (usage audit §12).
//!
//! Parses the [Fountain](https://fountain.io) plain-text screenplay format
//! into the suite's [`Document`] model. Every element maps 1:1 onto the
//! screenplay template's style ids — `SceneHeading`, `Action`, `Character`,
//! `Parenthetical`, `Dialogue`, `Transition`, and the title page's
//! `TitlePageTitle` / `TitlePageLine` — so a Fountain file opened against
//! that template's catalog lays out as a formatted screenplay.
//!
//! **The importer emits style references only.** The catalog that gives them
//! geometry is the caller's to merge (the editor merges the bundled
//! screenplay template's catalog; see `required_style_ids`), matching how the
//! OOXML/ODF importers leave catalog policy to their callers.
//!
//! Import-only, like EPUB is export-only: Fountain round-tripping is a
//! non-goal (the format cannot express most of the model).

#![forbid(unsafe_code)]

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;

mod classify;
mod emit;
mod inline;
mod strip;
mod title;

/// Errors from Fountain import.
#[derive(Debug, thiserror::Error)]
pub enum FountainError {
    /// The reader failed.
    #[error("I/O error reading Fountain input: {0}")]
    Io(#[from] std::io::Error),
    /// The input was not UTF-8 text.
    #[error("Fountain input is not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

/// The style ids the emitted document references. The caller must ensure its
/// catalog defines them (the bundled screenplay template does) or the
/// paragraphs fall back to default styling.
pub const REQUIRED_STYLE_IDS: &[&str] = &[
    "SceneHeading",
    "Action",
    "Character",
    "Parenthetical",
    "Dialogue",
    "Transition",
    "TitlePageTitle",
    "TitlePageLine",
];

/// Fountain importer (see the crate docs).
pub struct FountainImport;

impl DocumentImport for FountainImport {
    type Error = FountainError;
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

/// Parses Fountain source text into a [`Document`].
#[must_use]
pub fn parse_str(text: &str) -> Document {
    let text = strip::strip_boneyard_and_notes(text);
    let (title_blocks, body_src) = title::split_title_page(&text);
    let elements = classify::classify(body_src);
    let mut blocks = title_blocks;
    // A title page occupies page one; the first script element starts page
    // two via a direct page break — the screenplay template's own pattern.
    let break_first = !blocks.is_empty();
    emit::emit(&mut blocks, &elements, break_first);

    let mut doc = Document::new();
    doc.sections[0].blocks = blocks;
    doc
}
