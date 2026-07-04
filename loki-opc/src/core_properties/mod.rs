// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! OPC core properties (`/docProps/core.xml`) — Dublin Core document metadata
//! such as title, creator, and the created/modified timestamps. Parsing and
//! writing are gated behind the `serde` feature; without it the accessors are
//! no-op stubs.

use chrono::{DateTime, Utc};

#[cfg(feature = "serde")]
mod parse;
#[cfg(feature = "serde")]
mod write;

#[cfg(feature = "serde")]
pub use parse::parse_core_properties;
#[cfg(feature = "serde")]
pub use write::write_core_properties;

/// Parses the Core Properties part from XML bytes.
#[cfg(not(feature = "serde"))]
pub fn parse_core_properties(_xml: &[u8]) -> crate::error::OpcResult<CoreProperties> {
    Ok(CoreProperties::default())
}

/// Writes the Core Properties part to XML bytes.
#[cfg(not(feature = "serde"))]
pub fn write_core_properties(_props: &CoreProperties) -> crate::error::OpcResult<Vec<u8>> {
    Ok(Vec::new())
}

/// OPC core properties per §8.3.
/// All fields are optional; omit absent fields from serialized XML.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreProperties {
    /// Document category.
    pub category: Option<String>,
    /// Content status (e.g. "Draft", "Final").
    pub content_status: Option<String>,
    /// Creation timestamp (serialised as W3CDTF/ISO 8601).
    pub created: Option<DateTime<Utc>>,
    /// Name of the document's author.
    pub creator: Option<String>,
    /// Free-text description.
    pub description: Option<String>,
    /// Unique identifier for the document.
    pub identifier: Option<String>,
    /// Space- or comma-separated keywords.
    pub keywords: Option<String>,
    /// Document language tag (e.g. "en-US").
    pub language: Option<String>,
    /// Name of the user who last modified the document.
    pub last_modified_by: Option<String>,
    /// Timestamp of the last print.
    pub last_printed: Option<DateTime<Utc>>,
    /// Last-modification timestamp (serialised as W3CDTF/ISO 8601).
    pub modified: Option<DateTime<Utc>>,
    /// Revision number.
    pub revision: Option<String>,
    /// Document subject.
    pub subject: Option<String>,
    /// Document title.
    pub title: Option<String>,
    /// Version label.
    pub version: Option<String>,
}
