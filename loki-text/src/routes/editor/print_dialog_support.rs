// SPDX-License-Identifier: Apache-2.0

//! The Print dialog's pure helpers (split from `print_dialog.rs` for the
//! 300-line ceiling): the field-string → IPP options mapping and the
//! render-to-PDF step both routes share.

use std::sync::{Arc, Mutex};

use loki_print::{Duplex, PrintOptions};

use super::super::editor_publish::{PdfXLevelChoice, PublishFormat, serialize};
use crate::editing::state::DocumentState;

/// Builds the IPP job options from the dialog's field strings. Pure, so the
/// mapping is testable: copies parse loosely (blank = 1), the page-range
/// string is passed through verbatim — `loki-print` validates it segment by
/// segment at attribute-build time and refuses the job with a typed error
/// before any bytes reach a printer.
pub(in crate::routes::editor) fn build_ipp_options(
    copies: &str,
    pages: &str,
    duplex: bool,
    job_title: String,
) -> PrintOptions {
    PrintOptions {
        copies: copies.trim().parse().unwrap_or(1),
        duplex: if duplex {
            Duplex::LongEdge
        } else {
            Duplex::Simplex
        },
        media: None,
        color: loki_print::ColorMode::Auto,
        job_title: Some(job_title),
        page_ranges: {
            let trimmed = pages.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        },
    }
}

/// Renders the current document to print-ready PDF bytes (PDF/X-3, the
/// Publish tab's default level).
pub(super) fn render_pdf(doc_state: &Arc<Mutex<DocumentState>>) -> Result<Vec<u8>, String> {
    let doc = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.clone())
        .ok_or_else(|| "no document".to_string())?;
    serialize(&doc, PublishFormat::Pdf(PdfXLevelChoice::X3))
}
