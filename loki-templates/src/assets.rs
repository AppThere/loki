// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Embedded `.dotx` template assets.
//!
//! The bundled templates ship as real `.dotx` files (template content type) so
//! they are genuine, inspectable Word templates. They are regenerated from the
//! builders by the `gen_templates` binary and imported here at runtime. The
//! builders use only properties the DOCX round-trip preserves, so importing the
//! asset reproduces the authored template faithfully.

use std::io::Cursor;

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};

/// Raw bytes of the bundled `.dotx` for template `id`.
fn asset_bytes(id: &str) -> Option<&'static [u8]> {
    Some(match id {
        "markdown" => include_bytes!("../assets/markdown.dotx"),
        "apa" => include_bytes!("../assets/apa.dotx"),
        "mla" => include_bytes!("../assets/mla.dotx"),
        "screenplay" => include_bytes!("../assets/screenplay.dotx"),
        "resume" => include_bytes!("../assets/resume.dotx"),
        _ => return None,
    })
}

/// Imports template `id`'s bundled `.dotx` asset into a [`Document`].
pub(crate) fn document_from_asset(id: &str) -> Option<Document> {
    let bytes = asset_bytes(id)?;
    DocxImport::import(Cursor::new(bytes), DocxImportOptions::default()).ok()
}
