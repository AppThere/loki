// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OOXML namespace URIs and OPC relationship type strings.
//!
//! All constants are sourced from ECMA-376 5th edition and
//! ISO/IEC 29500-1:2016. Namespace URIs follow ECMA-376 Part 1 §L.5.

/// `WordprocessingML` main namespace (ECMA-376 §17.2).
pub const NS_WORDPROCESSINGML: &str =
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// OPC relationships namespace.
pub const NS_RELATIONSHIPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// `DrawingML` main namespace.
pub const NS_DRAWING: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

/// `DrawingML` `WordprocessingDrawing` namespace (ECMA-376 §20.4).
pub const NS_DRAWING_WP: &str =
    "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";

/// `DrawingML` Picture namespace (ECMA-376 §20.2).
pub const NS_DRAWING_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

/// Markup Compatibility namespace (ECMA-376 §15).
pub const NS_MARKUP_COMPAT: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

/// OPC relationship type for the main Office Document part.
pub const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";

/// OPC relationship type for the styles part (ECMA-376 §17.7).
pub const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";

/// OPC relationship type for the numbering definitions part (ECMA-376 §17.9).
pub const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";

/// OPC relationship type for the footnotes part (ECMA-376 §17.11.1).
pub const REL_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";

/// OPC relationship type for the endnotes part (ECMA-376 §17.11.1).
pub const REL_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";

/// OPC relationship type for the document settings part (ECMA-376 §17.15).
pub const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";

/// OPC relationship type for the comments part (ECMA-376 §17.13.4).
pub const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";

/// OPC relationship type for image parts (ECMA-376 §17.3.3.9).
pub const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

/// OPC relationship type for external hyperlink targets (ECMA-376 §17.16).
pub const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

/// OPC relationship type for the theme part.
pub const REL_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";

/// OPC relationship type for the header part (ECMA-376 §17.10).
pub const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";

/// OPC relationship type for the footer part (ECMA-376 §17.10).
pub const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
