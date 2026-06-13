// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document loading pipeline for the presentation editor.

use loki_file_access::FileAccessToken;
use loki_ooxml::pptx::import::{PptxImport, PptxImportOptions};
use loki_presentation_model::Presentation;

use crate::error::LoadError;
use crate::new_document;

/// Detected presentation format.
pub(super) enum DocumentFormat {
    Pptx,
    Odp,
    Unsupported(String),
}

/// Inspect the display name on `token` and return the [`DocumentFormat`].
pub(super) fn detect_format(token: &FileAccessToken) -> DocumentFormat {
    match token
        .display_name()
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("pptx") => DocumentFormat::Pptx,
        Some("odp") => DocumentFormat::Odp,
        Some(ext) => DocumentFormat::Unsupported(ext.to_string()),
        None => DocumentFormat::Unsupported(String::new()),
    }
}

/// Deserialise `path` → detect format → open → import → return a [`Presentation`].
///
/// Untitled paths yield a fresh single-slide deck. ODP import is not yet
/// implemented and reports [`LoadError::UnsupportedFormat`].
pub(super) fn load_presentation(path: String) -> Result<Presentation, LoadError> {
    if new_document::is_untitled(&path) {
        return Ok(blank_presentation());
    }

    let token = FileAccessToken::deserialize(&path)?;
    match detect_format(&token) {
        DocumentFormat::Pptx => {
            let reader = token.open_read()?;
            Ok(PptxImport::import(reader, PptxImportOptions::default())?)
        }
        DocumentFormat::Odp => Err(LoadError::UnsupportedFormat("odp".to_string())),
        DocumentFormat::Unsupported(ext) => Err(LoadError::UnsupportedFormat(ext)),
    }
}

/// A new, empty single-slide presentation.
fn blank_presentation() -> Presentation {
    use loki_presentation_model::Slide;
    let mut p = Presentation::default();
    p.add_slide(Slide::new("slide1", p.slide_size));
    p
}
