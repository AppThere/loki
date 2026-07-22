// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]

//! `loki-macro-sig` — macro **signature verification** for the trusted-publisher
//! tier (ADR-0014; macro spec §2.5, Phase 8 Track A).
//!
//! This crate answers one question about a preserved macro payload: **is it
//! signed, is the signature intact, and by whom** — returned as a total
//! [`SignatureVerdict`]. It is the isolated, security-critical crypto surface:
//! `#![forbid(unsafe_code)]`, pure Rust, no filesystem or network, fuzzed, and
//! (like every other reader) total — malformed or hostile input degrades to
//! [`SignatureVerdict::Invalid`], never a panic (T9).
//!
//! ## What this crate does *not* decide
//!
//! Verification proves **integrity + authorship**, never **trust**. A valid
//! signature by itself grants nothing — an attacker signs their own malware with
//! their own certificate. Trust is decided by the *caller* (`loki-macro-host`,
//! 8A.5) by matching the signer against a **user-pinned trusted-publisher store**
//! (ADR-0014 §4.3, T10). This crate only ever *reports* whether the signer's
//! thumbprint is one the caller told it to treat as trusted; it holds no policy.
//!
//! ## Status
//!
//! 8A.1 landed the output vocabulary ([`SignatureVerdict`], [`CertInfo`],
//! [`Thumbprint`], and the reason enums). 8A.2 (this drop) adds VBA
//! signature-stream location + discrimination ([`extract_vba_signatures`]) and
//! the embedded-PKCS#7 locator — parse only, fuzzed, total. Still to come:
//!
//! - 8A.3 (this drop) — PKCS#7 `SignedData` + X.509 verification
//!   ([`verify_signed_data`], `RustCrypto` stack), corpus-gated.
//! - 8A.4 — ODF `XMLDSig` (`macrosignatures.xml`).

mod odf;
mod odf_dom;
mod timestamp;
mod vba;
mod verdict;
mod verify;
mod verify_cert;
mod verify_crypto;
mod verify_odf;
mod xml_c14n;

pub use vba::{RawVbaSignature, SigVariant, extract_vba_signatures};
pub use verdict::{CertInfo, InvalidReason, SignatureVerdict, Thumbprint, UntrustedReason};
pub use verify::verify_signed_data;
pub use verify_odf::verify_xmldsig;
