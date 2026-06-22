// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document metadata export: populates the OPC core properties
//! (`docProps/core.xml`) from the format-neutral [`DocumentMeta`].
//!
//! The OPC layer (`loki-opc`) serialises the populated `CoreProperties`,
//! registers the part, its relationship, and content-type automatically during
//! `Package::write`. ECMA-376 §15.2.12.3 / ISO/IEC 29500-2 §11.

use loki_doc_model::meta::core::DocumentMeta;
use loki_opc::Package;

/// Copies the core document metadata from `meta` into `pkg`'s core properties
/// so they are written to `docProps/core.xml`.
///
/// The extended Dublin Core fields (publisher, contributors, …) have no home in
/// core.xml; only [`DocumentMeta::dublin_core`]'s `identifier` maps here (the
/// native `dc:identifier`). The remaining extended fields go to
/// `docProps/custom.xml` (see the `custom_props` module).
pub(super) fn populate_core_properties(pkg: &mut Package, meta: &DocumentMeta) {
    let cp = pkg.core_properties_mut();
    cp.title.clone_from(&meta.title);
    cp.subject.clone_from(&meta.subject);
    cp.keywords.clone_from(&meta.keywords);
    cp.description.clone_from(&meta.description);
    cp.creator.clone_from(&meta.creator);
    cp.last_modified_by.clone_from(&meta.last_modified_by);
    cp.created = meta.created;
    cp.modified = meta.modified;
    cp.language = meta.language.as_ref().map(|l| l.as_str().to_string());
    cp.revision = meta.revision.map(|r| r.to_string());
    cp.identifier.clone_from(&meta.dublin_core.identifier);
}
