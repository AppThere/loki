// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Canonicalization of document metadata — the core properties, custom
//! properties, and the extended Dublin Core set.
//!
//! Emitted under a `meta/…` path prefix so a dropped or mangled property
//! (title, creator, a `dcmi:` term, a custom field) surfaces as a first
//! divergence with a stable path, independent of document body content.
//! Dates use RFC 3339 so the comparison is textual and timezone-stable.

use loki_doc_model::meta::core::{CustomPropertyValue, DocumentMeta};

use super::push;
use crate::roundtrip::CanonicalEntry;

/// Walks `meta` into canonical `meta/…` entries (nothing for unset fields).
pub(super) fn canonicalize_meta(meta: &DocumentMeta, out: &mut Vec<CanonicalEntry>) {
    opt(out, "title", meta.title.as_deref());
    opt(out, "subject", meta.subject.as_deref());
    opt(out, "keywords", meta.keywords.as_deref());
    opt(out, "description", meta.description.as_deref());
    opt(out, "creator", meta.creator.as_deref());
    opt(out, "last_modified_by", meta.last_modified_by.as_deref());
    if let Some(d) = meta.created {
        push(out, "meta/created".to_string(), d.to_rfc3339());
    }
    if let Some(d) = meta.modified {
        push(out, "meta/modified".to_string(), d.to_rfc3339());
    }
    if let Some(l) = &meta.language {
        push(out, "meta/language".to_string(), l.as_str().to_string());
    }
    if let Some(r) = meta.revision {
        push(out, "meta/revision".to_string(), r.to_string());
    }
    if let Some(e) = meta.editing_duration_minutes {
        push(out, "meta/editing_minutes".to_string(), e.to_string());
    }
    // Custom properties, sorted by name so the entry order is stable.
    let mut custom: Vec<_> = meta.custom_properties.iter().collect();
    custom.sort_by(|a, b| a.name.cmp(&b.name));
    for p in custom {
        push(
            out,
            format!("meta/custom/{}", p.name),
            custom_value(&p.value),
        );
    }
    // Extended Dublin Core — already a flat, deterministic name/value list.
    for (name, value) in meta.dublin_core.to_named_pairs() {
        push(out, format!("meta/dc/{name}"), value);
    }
}

fn opt(out: &mut Vec<CanonicalEntry>, key: &str, v: Option<&str>) {
    if let Some(s) = v {
        push(out, format!("meta/{key}"), s.to_string());
    }
}

fn custom_value(v: &CustomPropertyValue) -> String {
    match v {
        CustomPropertyValue::Text(s) => s.clone(),
        CustomPropertyValue::Number(n) => format!("{n}"),
        CustomPropertyValue::Bool(b) => b.to_string(),
        CustomPropertyValue::DateTime(d) => d.to_rfc3339(),
        // `CustomPropertyValue` is `#[non_exhaustive]`; record any future
        // variant by a marker so its presence is still tracked.
        _ => "unknown".to_string(),
    }
}
