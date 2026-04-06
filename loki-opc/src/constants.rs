// Copyright 2024-2026 AppThere
//
// Licensed under the MIT License.

//! Namespace URIs and defined standard Media types mapped exactly into the registry specifications described under ISO/IEC 29500-2:2021 Annex E.

/// Identifies core properties configuration container
pub const CORE_PROPERTIES_NS: &str =
    "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";

/// Identifies relationships XML configuration
pub const RELATIONSHIPS_NS: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships";

/// Core metadata target relationships configuration
pub const REL_CORE_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";

/// Core thumbnail definitions targeting image representations
pub const REL_THUMBNAIL: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail";

/// Digital signature target configurations
pub const REL_DIGITAL_SIGNATURE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/signature";

/// Media type describing `.rels` containers specifically
pub const MEDIA_TYPE_RELATIONSHIPS: &str =
    "application/vnd.openxmlformats-package.relationships+xml";
