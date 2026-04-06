// Copyright 2024-2026 AppThere
//
// Licensed under the MIT License.

//! Handling metadata extraction identifying creation, update schedules comprehensively matching properties strictly.

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
    /// Categorical type descriptor.
    pub category: Option<String>,
    /// Editing revision status.
    pub content_status: Option<String>,
    /// Creation bounds mapping natively to W3CDTF chronological ISO.
    pub created: Option<DateTime<Utc>>,
    /// Identity string generating asset natively.
    pub creator: Option<String>,
    /// Contextual overview metadata segment.
    pub description: Option<String>,
    /// Structural identifier properties.
    pub identifier: Option<String>,
    /// Arbitrary tags defining metadata items.
    pub keywords: Option<String>,
    /// Semantic language boundary descriptions.
    pub language: Option<String>,
    /// Last editor name definitions recursively tracked across updates.
    pub last_modified_by: Option<String>,
    /// Chronological stamp for last physical export generation natively.
    pub last_printed: Option<DateTime<Utc>>,
    /// Mutational chronology tracked via standardized format constraints mapping internally.
    pub modified: Option<DateTime<Utc>>,
    /// Monotonic revision sequence number mapping file updates.
    pub revision: Option<String>,
    /// Metadata subject parameters identifying specific content targets natively.
    pub subject: Option<String>,
    /// Human-readable title defining root semantic scope comprehensively.
    pub title: Option<String>,
    /// Abstract configuration identifying parameters defining structural values explicitly tracking generation mapping bounds securely.
    pub version: Option<String>,
}
