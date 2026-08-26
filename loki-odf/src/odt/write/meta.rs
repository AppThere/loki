// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `meta.xml` writer for ODT (Dublin Core core + extended properties).
//!
//! The element shape lives in [`crate::meta_write`], shared with ODS; this
//! module only projects a [`Document`]'s metadata onto it.

use loki_doc_model::document::Document;

use crate::meta_write::{MetaFields, meta_xml_from};

/// Renders `meta.xml` for `doc` (Dublin Core core properties).
#[must_use]
pub(crate) fn meta_xml(doc: &Document) -> String {
    let m = &doc.meta;
    let fields = MetaFields {
        title: m.title.as_deref(),
        creator: m.creator.as_deref(),
        subject: m.subject.as_deref(),
        description: m.description.as_deref(),
        keywords: m.keywords.as_deref(),
        user_defined: m.dublin_core.to_named_pairs(),
    };
    meta_xml_from(&fields, super::xml::office_version(doc))
}
