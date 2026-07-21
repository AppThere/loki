// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The signature-verification output model (ADR-0014 §4.1). Verification is
//! **total**: every input maps to one [`SignatureVerdict`], so there is no
//! `Result` — a failure to parse or verify is the `Invalid`/`Unsigned` variant,
//! not an error. Only [`SignatureVerdict::ValidTrusted`] may raise trust, and
//! only the caller (against its pinned-publisher store) can produce it.

/// The SHA-256 thumbprint of a signer's DER-encoded certificate — the identity a
/// user pins in the trusted-publisher store (ADR-0014 §4.3). Comparing
/// thumbprints is how "is this signer trusted?" is answered, so equality is the
/// whole point of the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Thumbprint([u8; 32]);

impl Thumbprint {
    /// Wraps a 32-byte SHA-256 digest.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw 32-byte digest.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hex, for display and as the trusted-publisher store key.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            // Two lowercase hex digits per byte; `write!` to a String cannot fail.
            s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
            s.push(char::from_digit(u32::from(b & 0x0F), 16).unwrap_or('0'));
        }
        s
    }
}

/// Display + identity fields extracted from a signer certificate. All strings are
/// for **display only** (helping a user recognise a publisher before pinning);
/// trust is the [`Thumbprint`], never these fields (T10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertInfo {
    /// Subject common name (e.g. `"Contoso Ltd"`), for the headline label.
    pub subject_cn: String,
    /// Full subject distinguished name.
    pub subject: String,
    /// Issuer distinguished name (e.g. `"DigiCert Trusted G4 …"`).
    pub issuer: String,
    /// Certificate serial number, lowercase hex (identity/display).
    pub serial_hex: String,
    /// `notBefore`, Unix seconds (advisory display / expiry math).
    pub not_before: i64,
    /// `notAfter`, Unix seconds.
    pub not_after: i64,
    /// SHA-256 of the DER certificate — the trust anchor.
    pub thumbprint: Thumbprint,
}

/// Why an *intact* signature does not (yet) grant trust — a `ValidUntrusted`
/// verdict the UI turns into an explanation and, for [`Self::PublisherRenewed`],
/// a "trust the renewed certificate?" affordance (ADR-0014 §4.3–§4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UntrustedReason {
    /// Integrity is fine, but the signer is not in the trusted-publisher store.
    NotPinned,
    /// Signed only with a legacy (MD5-based) method Loki never trusts — the
    /// downgrade defence (ADR-0014 §4.2).
    LegacyAlgorithm,
    /// The signing certificate has expired and carries no trusted RFC-3161
    /// timestamp proving it was signed while valid (ADR-0014 §4.4).
    CertificateExpired,
    /// Same subject/issuer as a pinned publisher but a different thumbprint —
    /// almost certainly a certificate renewal; prompt before extending trust.
    PublisherRenewed,
}

/// Why a signature is **not intact** — an integrity/parse failure that makes the
/// signature meaningless, so it is treated as effectively unsigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidReason {
    /// The signature or certificate structures could not be parsed.
    Malformed,
    /// The signed message digest does not match the signed attributes.
    DigestMismatch,
    /// The signature covers content other than the source Loki would run
    /// (transplant / stale-signature attempt).
    ContentMismatch,
    /// A signature or digest algorithm this crate does not implement.
    UnsupportedAlgorithm,
}

/// The result of verifying a macro payload's signature (ADR-0014 §4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureVerdict {
    /// No macro signature was present.
    Unsigned,
    /// A signature was present but is not intact; treat as unsigned, but surface
    /// the reason so the user is not misled by a broken signature.
    Invalid(InvalidReason),
    /// Integrity + authorship verified, but trust is not granted (see reason).
    ValidUntrusted {
        /// The verified signer.
        signer: CertInfo,
        /// Why trust is withheld.
        reason: UntrustedReason,
    },
    /// Integrity + authorship verified **and** the signer matches a user-pinned
    /// trusted publisher. The only verdict that may enable macros at open
    /// (capability prompts still apply — signing proves origin, not safety).
    ValidTrusted {
        /// The verified signer.
        signer: CertInfo,
        /// The pinned thumbprint that matched (== `signer.thumbprint`).
        thumbprint: Thumbprint,
    },
}

impl SignatureVerdict {
    /// Whether a signature was present at all (intact or not).
    #[must_use]
    pub fn is_signed(&self) -> bool {
        !matches!(self, SignatureVerdict::Unsigned)
    }

    /// Whether this verdict authorises the trusted-publisher tier. Only
    /// `ValidTrusted` — never a bare valid signature.
    #[must_use]
    pub fn is_trusted(&self) -> bool {
        matches!(self, SignatureVerdict::ValidTrusted { .. })
    }

    /// The verified signer, if the signature was intact (`ValidUntrusted` /
    /// `ValidTrusted`); `None` for `Unsigned` / `Invalid`.
    #[must_use]
    pub fn signer(&self) -> Option<&CertInfo> {
        match self {
            SignatureVerdict::ValidUntrusted { signer, .. }
            | SignatureVerdict::ValidTrusted { signer, .. } => Some(signer),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "verdict_tests.rs"]
mod tests;
