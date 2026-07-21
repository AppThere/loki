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
//! 8A.1 lands the output vocabulary ([`SignatureVerdict`], [`CertInfo`],
//! [`Thumbprint`], and the reason enums) so the rest of the phase can be built
//! against a stable surface. The actual parsers and verifier land next:
//!
//! - 8A.2 — VBA signature-stream parsing (MS-OVBA / MS-OSHARED), fuzzed.
//! - 8A.3 — PKCS#7 `SignedData` + X.509 verification (`RustCrypto`), corpus-gated.
//! - 8A.4 — ODF `XMLDSig` (`macrosignatures.xml`).

mod verdict;

pub use verdict::{CertInfo, InvalidReason, SignatureVerdict, Thumbprint, UntrustedReason};
