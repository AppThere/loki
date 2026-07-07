// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT export writers: a [`loki_doc_model::document::Document`] is serialised to
//! the `content.xml`, `styles.xml`, and `meta.xml` parts of an ODT package,
//! along with any embedded image parts.

mod auto;
mod content;
mod inlines;
mod media;
mod page_styles;
mod para_props;
mod props;
mod revisions;
mod styles;
mod tables;
mod xml;

pub(crate) use content::content_xml;
pub(crate) use media::{MathPart, MediaPart};
pub(crate) use styles::{meta_xml, styles_xml};
