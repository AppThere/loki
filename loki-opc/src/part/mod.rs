// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Types and implementation of OPC package elements containing URI addressing handlers.

pub mod addressing;
mod name;

pub use name::PartName;

/// The data content of an OPC part.
#[derive(Debug, Clone)]
pub struct PartData {
    /// Raw bytes of the part content.
    pub bytes: Vec<u8>,
    /// The media type of this part.
    pub media_type: String,
    /// Optional growth hint (§6.2.4). May be 0.
    pub growth_hint: u64,
}

impl PartData {
    /// Constructs structural payload.
    pub fn new(bytes: Vec<u8>, media_type: impl Into<String>) -> Self {
        Self {
            bytes,
            media_type: media_type.into(),
            growth_hint: 0,
        }
    }

    /// Maps simple `xml` default strings automatically onto parsed parts.
    pub fn xml(bytes: Vec<u8>) -> Self {
        Self::new(bytes, "application/xml")
    }
}
