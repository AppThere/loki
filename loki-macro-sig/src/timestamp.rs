// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! RFC-3161 signature-timestamp handling for the CMS verifier (8A.6; ADR-0014
//! §4.4).
//!
//! A signed macro whose signing certificate has since **expired** may still be
//! honoured if it carries a timestamp proving it was signed while the
//! certificate was valid. The timestamp is an unsigned `SignerInfo` attribute
//! `id-aa-timeStampToken` (OID 1.2.840.113549.1.9.16.2.14) whose value is an
//! RFC-3161 `TimeStampToken` — a CMS `SignedData` over a `TSTInfo`.
//! [`signed_time`] extracts the token's `genTime`, but only after checking the
//! token's `messageImprint` **binds to this signature's octets** (so a timestamp
//! cannot be transplanted from another signature).
//!
//! **Scope / trust model.** This binds the timestamp to the signature and reads
//! its `genTime`; it does **not** verify the TSA's own signature or chain it to a
//! trusted timestamping root. That is deliberate and safe here because trust is
//! anchored on the signer's **pinned leaf thumbprint** (ADR-0014 §4.3), not a CA
//! chain: the timestamp-expiry path only rescues a signature whose *outer*
//! signature already verifies against a user-pinned publisher, so a forged
//! `genTime` cannot make attacker-controlled content trusted — only the pinned
//! publisher's own (genuinely signed) content is ever affected.
//! TODO(8A.6-tsa-anchor): if a chain-to-CA trust mode is ever added, the TSA must
//! then be verified and anchored to a trusted timestamping root.
//!
//! Total on hostile input: any parse/decode failure yields `None`, never a panic.

use cms::content_info::ContentInfo;
use cms::signed_data::{SignedData, SignerInfo};
use const_oid::ObjectIdentifier;
use const_oid::db::rfc5911::ID_SIGNED_DATA;
use der::asn1::{AnyRef, GeneralizedTime, OctetString};
use der::{Decode, Sequence, SliceReader, Tag, Tagged};
use spki::AlgorithmIdentifierOwned;

use crate::verify_crypto::DigestId;

/// `id-aa-timeStampToken` — the unsigned `SignerInfo` attribute carrying an
/// RFC-3161 timestamp token.
const ID_AA_TIMESTAMP_TOKEN: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.14");

/// RFC-3161 `MessageImprint ::= SEQUENCE { hashAlgorithm, hashedMessage }`.
#[derive(Sequence)]
struct MessageImprint {
    hash_algorithm: AlgorithmIdentifierOwned,
    hashed_message: OctetString,
}

/// The RFC-3161 `genTime` (Unix seconds) proven for `outer_signature`, or `None`
/// when there is no bound, well-formed timestamp.
///
/// `outer_signature` is the `SignerInfo` signature octets the timestamp is
/// computed over (RFC 3161 §2.4.2). The token's `messageImprint` must hash to
/// exactly those octets, or the timestamp is ignored.
pub(crate) fn signed_time(signer: &SignerInfo, outer_signature: &[u8]) -> Option<i64> {
    let unsigned = signer.unsigned_attrs.as_ref()?;
    let attr = unsigned.iter().find(|a| a.oid == ID_AA_TIMESTAMP_TOKEN)?;
    let token = attr.values.get(0)?;

    // The attribute value is a TimeStampToken: a CMS ContentInfo(SignedData)
    // whose eContent is the DER TSTInfo.
    let content_info: ContentInfo = token.decode_as().ok()?;
    if content_info.content_type != ID_SIGNED_DATA {
        return None;
    }
    let signed_data: SignedData = content_info.content.decode_as().ok()?;
    let econtent = signed_data.encap_content_info.econtent.as_ref()?;
    let tst_info = econtent.decode_as::<OctetString>().ok()?;

    let (digest, imprint, gen_time) = parse_tst_info(tst_info.as_bytes())?;
    // Binding: the timestamp must be over *this* signature, not a transplant.
    if digest.digest(outer_signature) != imprint {
        return None;
    }
    Some(gen_time)
}

/// Pulls `(imprint digest, imprint bytes, genTime seconds)` out of a `TSTInfo`.
///
/// `TSTInfo ::= SEQUENCE { version, policy, messageImprint, serialNumber,
/// genTime, … }` — the trailing optional fields (accuracy/ordering/nonce/tsa/
/// extensions) are not needed and left unread.
fn parse_tst_info(der_bytes: &[u8]) -> Option<(DigestId, Vec<u8>, i64)> {
    let any = AnyRef::from_der(der_bytes).ok()?;
    if any.tag() != Tag::Sequence {
        return None;
    }
    let mut reader = SliceReader::new(any.value()).ok()?;
    let _version = AnyRef::decode(&mut reader).ok()?;
    let _policy = AnyRef::decode(&mut reader).ok()?;
    let imprint = MessageImprint::decode(&mut reader).ok()?;
    let _serial = AnyRef::decode(&mut reader).ok()?;
    let gen_time = GeneralizedTime::decode(&mut reader).ok()?;

    let digest = DigestId::from_oid(&imprint.hash_algorithm.oid)?;
    let hashed = imprint.hashed_message.as_bytes().to_vec();
    let secs = i64::try_from(gen_time.to_unix_duration().as_secs()).ok()?;
    Some((digest, hashed, secs))
}

#[cfg(test)]
#[path = "timestamp_tests.rs"]
mod tests;
