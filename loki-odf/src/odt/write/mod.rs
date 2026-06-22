// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT export writers: a [`loki_doc_model::document::Document`] is serialised to
//! the `content.xml`, `styles.xml`, and `meta.xml` parts of an ODT package.

mod auto;
mod content;
mod props;
mod styles;
mod xml;

pub(crate) use content::content_xml;
pub(crate) use styles::{meta_xml, styles_xml};
