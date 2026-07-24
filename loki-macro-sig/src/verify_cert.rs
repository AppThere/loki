// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Signer-certificate resolution and [`CertInfo`] extraction for the CMS
//! verifier (8A.3). Pure, total: a malformed field yields an empty string or
//! `None`, never a panic.

use cms::cert::CertificateChoices;
use cms::signed_data::{SignedData, SignerIdentifier};
use const_oid::db::rfc4519::COMMON_NAME;
use der::Encode;
use der::asn1::{Ia5StringRef, PrintableStringRef, TeletexStringRef, Utf8StringRef};
use sha2::{Digest, Sha256};
use x509_cert::Certificate;
use x509_cert::attr::AttributeTypeAndValue;
use x509_cert::ext::pkix::SubjectKeyIdentifier;
use x509_cert::name::Name;

use crate::verdict::{CertInfo, Thumbprint};

/// Resolves the certificate a `SignerInfo` names via its `sid` — issuer + serial
/// number, or `SubjectKeyIdentifier` — from the `SignedData` certificate set.
/// `None` if no embedded certificate matches.
pub(crate) fn signer_cert<'a>(
    signed_data: &'a SignedData,
    sid: &SignerIdentifier,
) -> Option<&'a Certificate> {
    let certs = signed_data.certificates.as_ref()?;
    certs.0.iter().find_map(|choice| {
        let CertificateChoices::Certificate(cert) = choice else {
            return None;
        };
        if sid_matches(cert, sid) {
            Some(cert)
        } else {
            None
        }
    })
}

/// Whether `cert` is the one identified by `sid`.
fn sid_matches(cert: &Certificate, sid: &SignerIdentifier) -> bool {
    match sid {
        SignerIdentifier::IssuerAndSerialNumber(ias) => {
            cert.tbs_certificate.issuer == ias.issuer
                && cert.tbs_certificate.serial_number == ias.serial_number
        }
        SignerIdentifier::SubjectKeyIdentifier(ski) => {
            matches!(
                cert.tbs_certificate.get::<SubjectKeyIdentifier>(),
                Ok(Some((_, cert_ski))) if &cert_ski == ski
            )
        }
    }
}

/// Extracts the display + identity fields from a signer certificate. `None` only
/// if the certificate cannot be re-encoded to DER (needed for the thumbprint).
pub(crate) fn cert_info(cert: &Certificate) -> Option<CertInfo> {
    let der = cert.to_der().ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&der);
    let digest: [u8; 32] = hasher.finalize().into();

    let tbs = &cert.tbs_certificate;
    Some(CertInfo {
        subject_cn: common_name(&tbs.subject).unwrap_or_default(),
        subject: tbs.subject.to_string(),
        issuer: tbs.issuer.to_string(),
        serial_hex: serial_hex(tbs.serial_number.as_bytes()),
        not_before: unix_seconds(tbs.validity.not_before.to_unix_duration().as_secs()),
        not_after: unix_seconds(tbs.validity.not_after.to_unix_duration().as_secs()),
        thumbprint: Thumbprint::from_bytes(digest),
    })
}

/// The first `commonName` (OID 2.5.4.3) value in a distinguished name, if any.
fn common_name(name: &Name) -> Option<String> {
    name.0
        .iter()
        .flat_map(|rdn| rdn.0.iter())
        .find(|atv| atv.oid == COMMON_NAME)
        .and_then(attr_value_string)
}

/// Best-effort decode of a directory-string attribute value to a `String`,
/// trying the encodings X.500 names use in practice.
fn attr_value_string(atv: &AttributeTypeAndValue) -> Option<String> {
    let value = &atv.value;
    if let Ok(s) = value.decode_as::<Utf8StringRef<'_>>() {
        return Some(s.as_str().to_owned());
    }
    if let Ok(s) = value.decode_as::<PrintableStringRef<'_>>() {
        return Some(s.as_str().to_owned());
    }
    if let Ok(s) = value.decode_as::<Ia5StringRef<'_>>() {
        return Some(s.as_str().to_owned());
    }
    if let Ok(s) = value.decode_as::<TeletexStringRef<'_>>() {
        return Some(s.as_str().to_owned());
    }
    None
}

/// Lowercase hex of the serial-number bytes.
fn serial_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        s.push(char::from_digit(u32::from(b & 0x0F), 16).unwrap_or('0'));
    }
    s
}

/// Clamps a `u64` seconds-since-epoch into the `i64` [`CertInfo`] field.
fn unix_seconds(secs: u64) -> i64 {
    i64::try_from(secs).unwrap_or(i64::MAX)
}

#[cfg(test)]
#[path = "verify_cert_tests.rs"]
mod tests;
