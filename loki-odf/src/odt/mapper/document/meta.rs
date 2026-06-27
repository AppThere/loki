// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document-metadata mapping (`meta.xml` → [`DocumentMeta`]) and the shared
//! ISO-8601 datetime parser used by metadata and comment anchors.

use loki_doc_model::meta::core::DocumentMeta;

use crate::odt::model::document::OdfMeta;

pub(super) fn map_meta(meta: &OdfMeta) -> DocumentMeta {
    let keywords = if meta.keywords.is_empty() {
        None
    } else {
        Some(meta.keywords.join(", "))
    };
    DocumentMeta {
        title: meta.title.clone(),
        subject: meta.subject.clone(),
        keywords,
        description: meta.description.clone(),
        creator: meta.initial_creator.clone(),
        // ODF dc:creator is the person who last saved (= last_modified_by)
        last_modified_by: meta.creator.clone(),
        created: meta.created.as_deref().and_then(parse_datetime),
        modified: meta.modified.as_deref().and_then(parse_datetime),
        revision: meta.editing_cycles,
        dublin_core: loki_doc_model::meta::dublin_core::DublinCoreMeta::from_named_pairs(
            &meta.user_defined,
        ),
        ..Default::default()
    }
}

/// Parse an ISO-8601 / RFC-3339 datetime string into a UTC
/// [`chrono::DateTime`].
///
/// Tries RFC 3339 first (e.g. `"2024-01-15T10:30:00Z"`); falls back to
/// `"%Y-%m-%dT%H:%M:%S"` for strings without a timezone suffix.
pub(super) fn parse_datetime(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                .map(|ndt| ndt.and_utc())
                .ok()
        })
}
