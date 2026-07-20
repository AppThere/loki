// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Save-flow core for the macro editor (macro spec §3.4) — the pure part,
//! split from the component for the 300-line ceiling and unit testing.
//!
//! [`build_edited_payload`] applies edited module source to a copy of the
//! document's preserved payload, **source-only** (never p-code): VBA through
//! `loki_vba::write_source`, ODF Basic through `loki_odf::basic_write`. The
//! component then swaps the returned payload into the document and re-keys trust.

use std::collections::BTreeMap;

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};

/// Applies `edits` (module name → new LF source) to a copy of `original` and
/// returns the rewritten payload. Only the named modules change; for VBA the
/// whole `vbaProject.bin` is rebuilt source-only (other modules carried over,
/// p-code dropped), for ODF each named module's XML text is replaced.
///
/// # Errors
///
/// A diagnostic string (surfaced to `tracing`; the UI shows a localized
/// "couldn't save" message) if the container can't be rewritten or, for VBA, the
/// payload has no `vbaProject.bin` part.
pub(super) fn build_edited_payload(
    original: &MacroPayload,
    edits: &BTreeMap<String, String>,
) -> Result<MacroPayload, String> {
    let mut payload = original.clone();
    match payload.kind {
        MacroPayloadKind::OoxmlVba => {
            let (part_name, original_bin) = payload
                .parts
                .iter()
                .find(|p| p.name.ends_with("vbaProject.bin"))
                .map(|p| (p.name.clone(), p.bytes.clone()))
                .ok_or_else(|| "VBA payload has no vbaProject.bin part".to_string())?;
            let new_bin =
                loki_vba::write_source(&original_bin, edits).map_err(|e| e.to_string())?;
            payload.replace_part(&part_name, new_bin);
        }
        MacroPayloadKind::OdfBasic => {
            loki_odf::basic_write::apply_basic_edits(&mut payload, edits)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(payload)
}

/// The changed modules — pairs of (name, new source) where the draft differs
/// from the original module source. `originals` and `drafts` are index-aligned.
pub(super) fn changed_edits(
    names: &[String],
    originals: &[String],
    drafts: &[String],
) -> BTreeMap<String, String> {
    let mut edits = BTreeMap::new();
    for (name, (orig, draft)) in names.iter().zip(originals.iter().zip(drafts.iter())) {
        if orig != draft {
            edits.insert(name.clone(), draft.clone());
        }
    }
    edits
}

#[cfg(test)]
#[path = "editor_macro_editor_ops_tests.rs"]
mod tests;
