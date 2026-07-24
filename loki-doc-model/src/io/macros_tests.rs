// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;

fn part(name: &str, bytes: &[u8]) -> PreservedPart {
    PreservedPart::new(
        name,
        Some("application/octet-stream".into()),
        bytes.to_vec(),
    )
}

#[test]
fn empty_payload_reports_empty() {
    let p = MacroPayload::new(MacroPayloadKind::OoxmlVba, Vec::new());
    assert!(p.is_empty());
}

#[test]
fn hash_is_order_independent() {
    let a = MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![
            part("/word/vbaProject.bin", b"AAA"),
            part("/word/vbaData.xml", b"BBB"),
        ],
    );
    let b = MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![
            part("/word/vbaData.xml", b"BBB"),
            part("/word/vbaProject.bin", b"AAA"),
        ],
    );
    assert_eq!(a.payload_hash(), b.payload_hash());
}

#[test]
fn hash_changes_when_bytes_change() {
    let a = MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![part("/word/vbaProject.bin", b"AAA")],
    );
    let b = MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![part("/word/vbaProject.bin", b"AAB")],
    );
    assert_ne!(a.payload_hash(), b.payload_hash());
}

#[test]
fn hash_distinguishes_kinds() {
    let vba = MacroPayload::new(MacroPayloadKind::OoxmlVba, vec![part("x", b"AAA")]);
    let basic = MacroPayload::new(MacroPayloadKind::OdfBasic, vec![part("x", b"AAA")]);
    assert_ne!(vba.payload_hash(), basic.payload_hash());
}

#[test]
fn length_prefix_prevents_boundary_collision() {
    // Without length-prefixing, ("ab","c") and ("a","bc") would concatenate
    // identically. They must hash differently.
    let a = MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new("ab", None, b"c".to_vec())],
    );
    let b = MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new("a", None, b"bc".to_vec())],
    );
    assert_ne!(a.payload_hash(), b.payload_hash());
}

#[test]
fn event_bindings_do_not_affect_hash() {
    let base = MacroPayload::new(MacroPayloadKind::OoxmlVba, vec![part("m", b"AAA")]);
    let with_binding = base.clone().with_event_bindings(vec![RawEventBinding {
        event: "Document_Open".into(),
        target: Some("Module1.AutoOpen".into()),
    }]);
    assert_eq!(base.payload_hash(), with_binding.payload_hash());
}

#[test]
fn replace_part_updates_bytes_and_changes_hash() {
    let mut p = MacroPayload::new(
        MacroPayloadKind::OdfBasic,
        vec![part("Basic/Standard/Module1.xml", b"old")],
    );
    let before = p.payload_hash();

    assert!(p.replace_part("Basic/Standard/Module1.xml", b"new source".to_vec()));
    assert_eq!(p.parts[0].bytes, b"new source");
    // The media type and position are untouched.
    assert_eq!(
        p.parts[0].media_type.as_deref(),
        Some("application/octet-stream")
    );
    // Editing the content re-keys the trust hash (spec §2.4).
    assert_ne!(p.payload_hash(), before);
}

#[test]
fn replace_part_reports_missing() {
    let mut p = MacroPayload::new(MacroPayloadKind::OdfBasic, vec![part("a", b"x")]);
    let before = p.payload_hash();
    assert!(!p.replace_part("does-not-exist", b"y".to_vec()));
    assert_eq!(
        p.payload_hash(),
        before,
        "a no-op replace leaves the hash intact"
    );
}
