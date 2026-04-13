// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! ODF frame model types.
//!
//! A `draw:frame` (ODF 1.3 §10.4) is the container element for images,
//! text boxes, and other inline or anchored graphical objects. The frame
//! carries position and size attributes; its content is described by
//! [`OdfFrameKind`].

use super::paragraph::OdfParagraph;

/// An ODF drawing frame (`draw:frame`). ODF 1.3 §10.4.
///
/// Frames may be anchored to the page, a paragraph, or a character position.
/// All length attributes are stored as raw ODF strings (e.g. `"5.5cm"`).
#[derive(Debug, Clone)]
pub(crate) struct OdfFrame {
    /// `draw:name` — optional frame identifier.
    pub name: Option<String>,
    /// `draw:style-name` — graphic style reference.
    pub style_name: Option<String>,
    /// `text:anchor-type` — positioning model: `"paragraph"`, `"char"`,
    /// `"as-char"`, or `"page"`. ODF 1.3 §19.27.
    pub anchor_type: Option<String>,
    /// `svg:width` — frame width (e.g. `"10cm"`).
    pub width: Option<String>,
    /// `svg:height` — frame height.
    pub height: Option<String>,
    /// `svg:x` — horizontal offset from the anchor.
    pub x: Option<String>,
    /// `svg:y` — vertical offset from the anchor.
    pub y: Option<String>,
    /// The content kind of this frame.
    pub kind: OdfFrameKind,
}

/// The content carried by an [`OdfFrame`].
///
/// ODF 1.3 §10.4 — a frame can contain a `draw:image`, a `draw:text-box`,
/// or other graphical elements.
#[derive(Debug, Clone)]
pub(crate) enum OdfFrameKind {
    /// A linked or embedded image (`draw:image`). ODF 1.3 §10.5.
    Image {
        /// `xlink:href` — path within the package (e.g. `"Pictures/img.png"`)
        /// or an external URL.
        href: String,
        /// Media type of the image, if known (from the manifest or
        /// inferred from the extension).
        media_type: Option<String>,
        /// Content of the nested `svg:title` element, if present.
        title: Option<String>,
        /// Content of the nested `svg:desc` element, if present.
        desc: Option<String>,
    },

    /// A floating text box (`draw:text-box`). ODF 1.3 §10.7.
    TextBox {
        /// Paragraphs inside the text box.
        paragraphs: Vec<OdfParagraph>,
    },

    /// Any other frame content not specifically modelled above.
    Other,
}
