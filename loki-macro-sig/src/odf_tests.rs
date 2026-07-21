// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the structural `macrosignatures.xml` parse: it pulls out the
//! `SignedInfo`, references, signature value, and certificate without judging
//! algorithms, and is total on hostile input.

use super::parse_macro_signatures;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

const XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<document-signatures xmlns=\"urn:oasis:names:tc:opendocument:xmlns:digitalsignature:1.0\">\
<Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\" Id=\"ID_1\">\
<SignedInfo>\
<CanonicalizationMethod Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\"></CanonicalizationMethod>\
<SignatureMethod Algorithm=\"http://www.w3.org/2001/04/xmldsig-more#rsa-sha256\"></SignatureMethod>\
<Reference URI=\"Basic/Standard/Module1.xml\">\
<DigestMethod Algorithm=\"http://www.w3.org/2001/04/xmlenc#sha256\"></DigestMethod>\
<DigestValue>YWJj</DigestValue>\
</Reference>\
<Reference URI=\"#idSignedProperties\">\
<DigestMethod Algorithm=\"http://www.w3.org/2001/04/xmlenc#sha256\"></DigestMethod>\
<DigestValue>ZGVm</DigestValue>\
</Reference>\
</SignedInfo>\
<SignatureValue>Z2hp</SignatureValue>\
<KeyInfo><X509Data><X509Certificate>amts</X509Certificate></X509Data></KeyInfo>\
</Signature></document-signatures>";

#[test]
fn extracts_the_signature_structure() {
    let sigs = parse_macro_signatures(XML.as_bytes());
    assert_eq!(sigs.len(), 1);
    let s = &sigs[0];

    assert_eq!(
        s.signature_method_uri,
        "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256"
    );
    assert_eq!(s.signature_value, STANDARD.decode("Z2hp").unwrap());
    assert_eq!(s.cert_der, STANDARD.decode("amts").unwrap());

    assert_eq!(s.references.len(), 2);
    assert_eq!(s.references[0].uri, "Basic/Standard/Module1.xml");
    assert_eq!(
        s.references[0].digest_value,
        STANDARD.decode("YWJj").unwrap()
    );
    assert_eq!(s.references[1].uri, "#idSignedProperties");
}

#[test]
fn signed_info_apex_carries_the_flattened_default_namespace() {
    let sigs = parse_macro_signatures(XML.as_bytes());
    let si = &sigs[0].signed_info;
    assert_eq!(si.name.local, "SignedInfo");
    // The dsig default namespace (declared on <Signature>) is flattened on.
    assert!(
        si.ns_decls
            .contains(&(None, "http://www.w3.org/2000/09/xmldsig#".to_owned()))
    );
}

#[test]
fn no_signatures_for_non_xml_or_empty_document() {
    assert!(parse_macro_signatures(b"not xml").is_empty());
    assert!(parse_macro_signatures(b"").is_empty());
    let empty = "<document-signatures xmlns=\"urn:oasis:names:tc:opendocument:xmlns:digitalsignature:1.0\"></document-signatures>";
    assert!(parse_macro_signatures(empty.as_bytes()).is_empty());
}

#[test]
fn signature_without_signed_info_is_skipped() {
    let xml = "<document-signatures><Signature><SignatureValue>Z2hp</SignatureValue></Signature></document-signatures>";
    assert!(parse_macro_signatures(xml.as_bytes()).is_empty());
}
