// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Error types for the `loki-doc-model` crate.
//!
//! Errors are produced when constructing or validating document model elements.
//! Format-specific errors (ODF parse errors, OOXML parse errors) are defined
//! in `loki-odf` and `loki-ooxml` respectively; this module covers only
//! model-level validation.

use thiserror::Error;

/// Errors that can occur when working with document model types.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LokiDocError {
    /// A style identifier referenced by content is not present in the
    /// [`crate::style::StyleCatalog`].
    #[error("style '{0}' not found in the style catalog")]
    StyleNotFound(String),

    /// A list identifier referenced by a paragraph's [`crate::style::props::ParaProps`]
    /// is not present in the style catalog.
    #[error("list style '{0}' not found in the style catalog")]
    ListStyleNotFound(String),

    /// A heading level outside the valid range 1–6 was specified.
    ///
    /// Corresponds to pandoc's `Header` level constraint.
    #[error("heading level {0} is out of range; valid range is 1–6")]
    InvalidHeadingLevel(u8),

    /// A list nesting level outside the valid range 0–8 was specified.
    ///
    /// Corresponds to TR 29166 §7.2.5 which defines up to 9 list levels
    /// (0-indexed 0–8).
    #[error("list level {0} is out of range; valid range is 0–8")]
    InvalidListLevel(u8),

    /// A page size dimension is not positive.
    #[error("page dimension must be positive; got {0}")]
    InvalidPageDimension(String),

    /// A BCP 47 language tag is malformed.
    ///
    /// See [`crate::meta::LanguageTag`].
    #[error("malformed BCP 47 language tag: '{0}'")]
    MalformedLanguageTag(String),
}
