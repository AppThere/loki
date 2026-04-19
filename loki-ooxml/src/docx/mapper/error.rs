// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Typed error enum for the OOXML → doc-model mapper layer.

use thiserror::Error;

/// Errors that can occur while mapping the OOXML intermediate model to
/// [`loki_doc_model`].
///
/// Optional or enrichment-only elements are never the source of an error;
/// they map to `None` / a default value instead. Only structurally required
/// elements that are absent or carry an invalid value produce a `MapperError`.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum MapperError {
    /// A required OOXML element was absent in the intermediate model.
    ///
    /// # Example
    ///
    /// ```
    /// use loki_ooxml::docx::mapper::MapperError;
    ///
    /// let e = MapperError::MissingRequiredElement { element: "w:body" };
    /// assert!(e.to_string().contains("w:body"));
    /// ```
    #[error("missing required OOXML element: {element}")]
    MissingRequiredElement {
        /// The OOXML element name (e.g. `"w:body"`, `"w:sectPr"`).
        element: &'static str,
    },

    /// An OOXML element was present but carried an invalid or unsupported value.
    ///
    /// # Example
    ///
    /// ```
    /// use loki_ooxml::docx::mapper::MapperError;
    ///
    /// let e = MapperError::InvalidValue {
    ///     element: "w:pgSz",
    ///     detail: "width must be positive".into(),
    /// };
    /// assert!(e.to_string().contains("w:pgSz"));
    /// ```
    #[error("invalid value for OOXML element {element}: {detail}")]
    InvalidValue {
        /// The OOXML element name where the invalid value was found.
        element: &'static str,
        /// A human-readable description of what was wrong.
        detail: String,
    },

    /// An error occurred in the OPC-packaging or XML-parsing layer before
    /// mapping could begin.
    ///
    /// This variant is produced by [`super::map_document`] when the OPC
    /// package is malformed or a required DOCX part cannot be parsed.
    #[error("OOXML pipeline error: {0}")]
    Pipeline(#[from] crate::error::OoxmlError),
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_required_element_display() {
        let e = MapperError::MissingRequiredElement { element: "w:body" };
        assert!(e.to_string().contains("w:body"));
    }

    #[test]
    fn invalid_value_display() {
        let e = MapperError::InvalidValue {
            element: "w:pgSz",
            detail: "width must be positive".into(),
        };
        let s = e.to_string();
        assert!(s.contains("w:pgSz"));
        assert!(s.contains("width must be positive"));
    }
}
