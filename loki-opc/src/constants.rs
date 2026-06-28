// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! Well-known OPC namespace URIs, relationship types, and media types
//! (ISO/IEC 29500-2:2021).

/// XML namespace for the core-properties part.
pub const CORE_PROPERTIES_NS: &str =
    "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";

/// XML namespace for relationships (`.rels`) parts.
pub const RELATIONSHIPS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// Relationship type for the core-properties part.
pub const REL_CORE_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";

/// Relationship type for the package thumbnail.
pub const REL_THUMBNAIL: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail";

/// Relationship type for a digital-signature part.
pub const REL_DIGITAL_SIGNATURE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/signature";

/// Media type for relationships (`.rels`) parts.
pub const MEDIA_TYPE_RELATIONSHIPS: &str =
    "application/vnd.openxmlformats-package.relationships+xml";
