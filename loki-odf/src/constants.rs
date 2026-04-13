// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! ODF namespace URIs, MIME types, and version strings.
//!
//! All namespace URIs are sourced from the OASIS ODF 1.3 specification
//! (OASIS Standard 2021) and its predecessors. Constant names follow the
//! prefix convention used in ODF documents (e.g. `NS_OFFICE` for
//! `xmlns:office="…"`).

/// ODF office namespace (ODF 1.3 §19.2).
pub const NS_OFFICE: &str = "urn:oasis:names:tc:opendocument:xmlns:office:1.0";

/// ODF text namespace (ODF 1.3 §19.2).
pub const NS_TEXT: &str = "urn:oasis:names:tc:opendocument:xmlns:text:1.0";

/// ODF style namespace (ODF 1.3 §19.2).
pub const NS_STYLE: &str = "urn:oasis:names:tc:opendocument:xmlns:style:1.0";

/// ODF XSL-FO-compatible namespace for formatting properties (ODF 1.3 §19.2).
pub const NS_FO: &str =
    "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0";

/// ODF table namespace (ODF 1.3 §19.2).
pub const NS_TABLE: &str = "urn:oasis:names:tc:opendocument:xmlns:table:1.0";

/// ODF drawing namespace (ODF 1.3 §19.2).
pub const NS_DRAW: &str = "urn:oasis:names:tc:opendocument:xmlns:drawing:1.0";

/// ODF SVG-compatible namespace (ODF 1.3 §19.2).
pub const NS_SVG: &str =
    "urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0";

/// XLink namespace used for hyperlinks and image references (ODF 1.3 §19.2).
pub const NS_XLINK: &str = "http://www.w3.org/1999/xlink";

/// Dublin Core namespace for document metadata (ODF 1.3 §19.2).
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";

/// ODF metadata namespace (ODF 1.3 §19.2).
pub const NS_META: &str = "urn:oasis:names:tc:opendocument:xmlns:meta:1.0";

/// ODF data-style (number) namespace (ODF 1.3 §19.2).
pub const NS_NUMBER: &str =
    "urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0";

/// ODF manifest namespace used in `META-INF/manifest.xml` (ODF 1.3 §3.3).
pub const NS_MANIFEST: &str =
    "urn:oasis:names:tc:opendocument:xmlns:manifest:1.0";

// ── MIME type ─────────────────────────────────────────────────────────────────

/// MIME type for an ODT (OpenDocument Text) package.
///
/// This string must appear verbatim as the first entry in the ZIP archive,
/// uncompressed, with no trailing newline. ODF 1.3 §3.3.
pub const MIME_ODT: &str = "application/vnd.oasis.opendocument.text";

// ── ODF version strings ────────────────────────────────────────────────────────

/// Version string for ODF 1.1 (ISO/IEC 26300:2006/Amd 1:2012).
pub const VERSION_1_1: &str = "1.1";

/// Version string for ODF 1.2 (ISO/IEC 26300-1:2015).
pub const VERSION_1_2: &str = "1.2";

/// Version string for ODF 1.3 (OASIS Standard 2021).
pub const VERSION_1_3: &str = "1.3";

/// Default version string used when creating new documents programmatically.
///
/// New documents default to ODF 1.3 unless the caller specifies otherwise.
pub const VERSION_DEFAULT: &str = VERSION_1_3;

// ── Package entry names ────────────────────────────────────────────────────────

/// ZIP entry name for the mandatory mimetype marker. ODF 1.3 §3.3.
pub const ENTRY_MIMETYPE: &str = "mimetype";

/// ZIP entry name for the manifest. ODF 1.3 §3.3.
pub const ENTRY_MANIFEST: &str = "META-INF/manifest.xml";

/// ZIP entry name for the document content. ODF 1.3 §3.1.
pub const ENTRY_CONTENT: &str = "content.xml";

/// ZIP entry name for document styles. ODF 1.3 §3.1.
pub const ENTRY_STYLES: &str = "styles.xml";

/// ZIP entry name for document metadata. ODF 1.3 §3.1.
pub const ENTRY_META: &str = "meta.xml";

/// ZIP entry name for application settings. ODF 1.3 §3.1.
pub const ENTRY_SETTINGS: &str = "settings.xml";
