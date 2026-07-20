// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Preserved macro/script payloads (provenance layer).
//!
//! Real-world office documents carry executable content: VBA projects in
//! OOXML macro-enabled formats (`.docm`/`.xlsm`/…) and `StarBasic` script
//! libraries in ODF packages. Loki does **not** execute these in Phase 1;
//! it *preserves* them byte-for-byte so a load→edit→save cycle no longer
//! silently destroys them.
//!
//! Per [`LOKI_MACRO_SCRIPTING_SPEC`] §3.2, the payload attaches to the
//! provenance layer ([`super::source::DocumentSource`]) — **not** to the
//! document body and **not** to the Loro CRDT. Nothing in this module
//! executes, interprets, or trusts the bytes it carries; it is inert
//! storage plus a canonical content hash used later as the trust-store key
//! (spec §2.4).
//!
//! [`LOKI_MACRO_SCRIPTING_SPEC`]: ../../../docs/adr/LOKI_MACRO_SCRIPTING_SPEC.md

use sha2::{Digest, Sha256};

/// Which macro/script family a [`MacroPayload`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MacroPayloadKind {
    /// OOXML VBA project (`vbaProject.bin` + `vbaData.xml`), CFB-encoded.
    OoxmlVba,
    /// ODF `StarBasic`/Basic script libraries (`Basic/`, `Scripts/`,
    /// `<office:scripts>` bindings).
    OdfBasic,
}

/// A single container part preserved verbatim.
///
/// The `bytes` are opaque: no parsing, decompression, or validation is
/// performed on them at the model layer. `name` is the format-native part
/// path (OOXML part name like `/word/vbaProject.bin`, or ODF ZIP entry like
/// `Basic/Standard/Module1.xml`); `media_type` is the content-type override
/// or manifest media type where the format records one.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PreservedPart {
    /// Format-native part path / ZIP entry name.
    pub name: String,
    /// Recorded media type, if the format supplies one for this part.
    pub media_type: Option<String>,
    /// Raw bytes, preserved verbatim.
    pub bytes: Vec<u8>,
}

impl PreservedPart {
    /// Creates a preserved part.
    #[must_use]
    pub fn new(name: impl Into<String>, media_type: Option<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            media_type,
            bytes,
        }
    }
}

/// A macro auto-execution binding detected at import, kept for UI and
/// warning purposes **only** (spec §5.6, §9).
///
/// This is descriptive metadata — the presence of a `Document_Open` or
/// ODF `OnLoad` listener is surfaced to the user so the security UI can
/// explain *why* a document wants to run code on open. It never drives
/// execution; Phase 1 has no execution surface at all.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawEventBinding {
    /// The event name as recorded by the format (e.g. `"Document_Open"`,
    /// `"OnLoad"`, `"Auto_Open"`).
    pub event: String,
    /// The macro/script the binding targets, if named by the format.
    pub target: Option<String>,
}

/// A preserved, inert macro/script payload attached to a document's
/// provenance.
///
/// Held on [`super::source::DocumentSource`]. Importers populate it;
/// exporters re-emit [`Self::parts`] verbatim when writing back to a
/// macro-capable format, or drop it (with a warning) when the target format
/// cannot carry it (spec §3.3, §3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MacroPayload {
    /// Which family this payload belongs to.
    pub kind: MacroPayloadKind,
    /// The preserved container parts, in a stable order.
    pub parts: Vec<PreservedPart>,
    /// Auto-execution bindings detected at import (advisory; spec §5.6).
    pub event_bindings: Vec<RawEventBinding>,
}

impl MacroPayload {
    /// Creates a payload from its parts, with no detected event bindings.
    #[must_use]
    pub fn new(kind: MacroPayloadKind, parts: Vec<PreservedPart>) -> Self {
        Self {
            kind,
            parts,
            event_bindings: Vec::new(),
        }
    }

    /// Builder: attach detected auto-run event bindings.
    #[must_use]
    pub fn with_event_bindings(mut self, bindings: Vec<RawEventBinding>) -> Self {
        self.event_bindings = bindings;
        self
    }

    /// Returns `true` if the payload carries no parts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Replaces the bytes of the part named `name`, returning `true` if such a
    /// part existed and was updated (its media type and position are unchanged),
    /// or `false` if no part matched. Used by the macro editor's source-only
    /// write-back (spec §3.4): after editing, the caller rewrites the affected
    /// container part in place, and [`payload_hash`](Self::payload_hash)
    /// recomputes to the new content — the value the trust store is then
    /// re-keyed to for a self-authored edit (§2.4).
    pub fn replace_part(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        match self.parts.iter_mut().find(|p| p.name == name) {
            Some(part) => {
                part.bytes = bytes;
                true
            }
            None => false,
        }
    }

    /// Computes the canonical content hash used as the trust-store key
    /// (spec §2.4).
    ///
    /// The hash is a deterministic function of the payload's *content* —
    /// the kind and every part's name, media type, and bytes — and is
    /// **independent of part ordering** (parts are sorted by name before
    /// hashing) and of anything outside the payload (file path, timestamps,
    /// the rest of the document). Two documents with byte-identical macro
    /// content therefore share a key, so renaming or copying a trusted file
    /// keeps trust while *changing the macros* revokes it.
    ///
    /// `event_bindings` are derived from the parts, so they are intentionally
    /// excluded from the hash to avoid double-counting.
    #[must_use]
    pub fn payload_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        // Domain separation + kind, so the two families never collide.
        hasher.update(b"loki-macro-payload\x00");
        let kind_tag: u8 = match self.kind {
            MacroPayloadKind::OoxmlVba => 1,
            MacroPayloadKind::OdfBasic => 2,
        };
        hasher.update([kind_tag]);

        let mut ordered: Vec<&PreservedPart> = self.parts.iter().collect();
        ordered.sort_by(|a, b| a.name.cmp(&b.name));
        for part in ordered {
            // Length-prefix every field so no concatenation ambiguity can
            // let two distinct payloads hash the same.
            write_len_prefixed(&mut hasher, part.name.as_bytes());
            match &part.media_type {
                Some(mt) => {
                    hasher.update([1u8]);
                    write_len_prefixed(&mut hasher, mt.as_bytes());
                }
                None => hasher.update([0u8]),
            }
            write_len_prefixed(&mut hasher, &part.bytes);
        }
        hasher.finalize().into()
    }
}

fn write_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
#[path = "macros_tests.rs"]
mod tests;
