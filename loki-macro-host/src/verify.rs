// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Verifying a preserved [`MacroPayload`]'s signature (8A.8; ADR-0014 §4.5).
//!
//! This bridges the format-neutral [`MacroPayload`] (opaque preserved parts) and
//! the `loki-macro-sig` verifier: it locates the signature within the payload and
//! feeds the verifier the exact source bytes it must hash, returning a total
//! [`SignatureVerdict`]. Trust is *not* decided here — the caller resolves the
//! verdict against the pinned-publisher store (8A.5).

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};
use loki_macro_sig::{SignatureVerdict, verify_xmldsig};

/// The ODF part that anchors macro trust (`documentsignatures.xml` is display
/// only, ADR-0014 §4.5).
const ODF_MACRO_SIG: &str = "META-INF/macrosignatures.xml";

/// Verifies the signature preserved in `payload`, returning a total verdict.
///
/// ODF (`OdfBasic`) is verified end-to-end: the `macrosignatures.xml` XMLDSig is
/// checked against the other preserved parts (the exact bytes a runner would
/// execute). VBA (`OoxmlVba`) verification is not yet wired — it needs the
/// MS-OVBA V3/agile content-hash of the project (`TODO(8A.3-corpus)`); until then
/// a signed VBA project reads [`SignatureVerdict::Unsigned`] rather than a
/// misleading `Invalid`.
#[must_use]
pub fn verify_payload(payload: &MacroPayload) -> SignatureVerdict {
    match payload.kind {
        MacroPayloadKind::OdfBasic => verify_odf(payload),
        // TODO(8A.8-vba-content): wire the MS-OVBA signed-content hash so VBA
        // signatures verify; until then a signed VBA project reads as Unsigned.
        MacroPayloadKind::OoxmlVba => SignatureVerdict::Unsigned,
    }
}

/// Verifies an ODF `macrosignatures.xml` against the payload's other parts. The
/// resolver maps a reference URI (a package path like
/// `Basic/Standard/Module1.xml`) to the preserved bytes for that part.
///
/// The signature must cover **every executable script part** — each non-empty
/// `Basic/`/`Scripts/` file the runner could execute — or it is rejected: a
/// signature that leaves a module unreferenced does not vouch for the code that
/// runs (ADR-0014 §4.5; the fail-closed reading pending corpus validation,
/// `TODO(8A.4-corpus)`).
fn verify_odf(payload: &MacroPayload) -> SignatureVerdict {
    let Some(sig) = payload.parts.iter().find(|p| p.name == ODF_MACRO_SIG) else {
        return SignatureVerdict::Unsigned;
    };
    let require_covered: Vec<&str> = payload
        .parts
        .iter()
        .filter(|p| is_executable_script_part(&p.name) && !p.bytes.is_empty())
        .map(|p| p.name.as_str())
        .collect();
    verify_xmldsig(&sig.bytes, &require_covered, |uri| {
        payload
            .parts
            .iter()
            .find(|p| p.name == uri)
            .map(|p| p.bytes.clone())
    })
}

/// Whether `name` is an executable ODF macro/script file (a `Basic/`/`Scripts/`
/// file entry, not a directory). These are the parts a signature must cover.
fn is_executable_script_part(name: &str) -> bool {
    (name.starts_with("Basic/") || name.starts_with("Scripts/")) && !name.ends_with('/')
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
