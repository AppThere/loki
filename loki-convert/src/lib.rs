// SPDX-License-Identifier: Apache-2.0

//! Format-conversion matrix (headless spec ADR-C024).
//!
//! Conversion is orchestration, not new format code: every pair goes
//! *import source → in-memory model → export target* through the existing
//! `loki-ooxml` / `loki-odf` / `loki-epub` / `loki-pdf` crates, so
//! conversion quality is exactly the round-trip quality those crates
//! already measure. Unsupported pairs are a typed
//! [`ConvertError::ConversionUnsupported`], never a lossy best-effort;
//! PPTX (and ODP/ODG) stay gated until the ACID PPTX generator lands
//! (ratified decision §5.1).

#![forbid(unsafe_code)]

mod error;
mod format;
mod matrix;
mod pipeline;

pub use error::ConvertError;
pub use format::{Format, PdfProfile};
pub use matrix::{is_supported, supported_pairs};
pub use pipeline::{ConvertOptions, ConvertOutput, convert};
