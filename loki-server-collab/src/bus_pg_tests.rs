// SPDX-License-Identifier: Apache-2.0

//! `PgNotifyBus` envelope tests (F-CO-2).
//!
//! Postgres itself is out of scope here — what is covered is everything the
//! bus does *around* the connection: the `NOTIFY` envelope wire format, the
//! awareness size guard, the size budget the guard's constant is derived
//! from, and the fan-in decision for one received payload.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use loki_model::{DocumentId, UserId};
use loki_server_store::OplogStore as _;
use loki_server_store::memory::MemoryStores;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use super::*;

/// This process' instance id.
const OURS: Uuid = Uuid::from_u128(1);
/// Another instance's id.
const THEIRS: Uuid = Uuid::from_u128(2);

/// Postgres' `NOTIFY` payload limit (8000 bytes; the documented cap is
/// `NAMEDATALEN`-relative but 8000 is the practical byte budget).
const NOTIFY_BUDGET_BYTES: usize = 8000;

fn origin(instance: Uuid, conn: u64) -> Origin {
    Origin { instance, conn }
}

// ---------------------------------------------------------------------------
// Envelope wire format
// ---------------------------------------------------------------------------

/// The envelope crosses process (and potentially version) boundaries, so the
/// exact JSON is the contract — a round-trip alone would survive renaming
/// `kind`, un-flattening `body`, or changing the variant casing.
#[test]
fn update_notification_serializes_to_the_pinned_wire_form() {
    let json = serde_json::to_string(&Notification {
        instance: OURS,
        conn: 7,
        doc: Uuid::from_u128(3),
        body: Body::Update { seq: 42 },
    })
    .unwrap();

    assert_eq!(
        json,
        r#"{"instance":"00000000-0000-0000-0000-000000000001","conn":7,"doc":"00000000-0000-0000-0000-000000000003","kind":"update","seq":42}"#
    );

    // …and the same bytes parse back into the same envelope.
    let back: Notification = serde_json::from_str(&json).unwrap();
    assert_eq!(back.instance, OURS);
    assert_eq!(back.conn, 7);
    assert_eq!(back.doc, Uuid::from_u128(3));
    assert!(matches!(back.body, Body::Update { seq: 42 }));
}

#[test]
fn awareness_notification_serializes_to_the_pinned_wire_form() {
    let json = serde_json::to_string(&Notification {
        instance: THEIRS,
        conn: 0,
        doc: Uuid::from_u128(3),
        body: Body::Awareness {
            data: BASE64.encode(b"hi"),
        },
    })
    .unwrap();

    assert_eq!(
        json,
        r#"{"instance":"00000000-0000-0000-0000-000000000002","conn":0,"doc":"00000000-0000-0000-0000-000000000003","kind":"awareness","data":"aGk="}"#
    );

    let back: Notification = serde_json::from_str(&json).unwrap();
    match back.body {
        Body::Awareness { data } => assert_eq!(BASE64.decode(data).unwrap(), b"hi"),
        Body::Update { .. } => panic!("kind tag decoded to the wrong variant"),
    }
}

/// The tag is what distinguishes the two bodies; an envelope without it (or
/// with an unknown one) must not silently decode as either variant.
#[test]
fn notifications_without_a_known_kind_tag_are_rejected() {
    for json in [
        r#"{"instance":"00000000-0000-0000-0000-000000000002","conn":0,"doc":"00000000-0000-0000-0000-000000000003","seq":1}"#,
        r#"{"instance":"00000000-0000-0000-0000-000000000002","conn":0,"doc":"00000000-0000-0000-0000-000000000003","kind":"delete","seq":1}"#,
    ] {
        assert!(
            serde_json::from_str::<Notification>(json).is_err(),
            "{json}"
        );
    }
}

// ---------------------------------------------------------------------------
// Size budget
// ---------------------------------------------------------------------------

/// Serialized length of a worst-case awareness envelope carrying `payload`
/// raw bytes: max-width uuids and a max-width connection id.
fn envelope_len(payload: usize) -> usize {
    serde_json::to_string(&Notification {
        instance: Uuid::from_u128(u128::MAX),
        conn: u64::MAX,
        doc: Uuid::from_u128(u128::MAX),
        body: Body::Awareness {
            data: BASE64.encode(vec![0xFF; payload]),
        },
    })
    .unwrap()
    .len()
}

/// `MAX_AWARENESS_BYTES` exists to keep base64 expansion plus the envelope
/// under Postgres' `NOTIFY` cap. Asserted from both sides so the constant
/// cannot drift up past the budget, and is not merely a small number that
/// would pass no matter what.
#[test]
fn awareness_limit_is_derived_from_the_notify_budget() {
    assert!(
        envelope_len(MAX_AWARENESS_BYTES) <= NOTIFY_BUDGET_BYTES,
        "a max-size awareness envelope is {} bytes, over the {NOTIFY_BUDGET_BYTES}-byte NOTIFY budget",
        envelope_len(MAX_AWARENESS_BYTES)
    );
    assert!(
        envelope_len(MAX_AWARENESS_BYTES * 2) > NOTIFY_BUDGET_BYTES,
        "twice the limit still fits the budget — the constant is not the binding constraint"
    );
}

// ---------------------------------------------------------------------------
// The awareness size guard (the only producer of `AwarenessTooLarge`)
// ---------------------------------------------------------------------------

/// A bus whose pool points at a closed port. Everything up to the `NOTIFY`
/// runs for real and the `NOTIFY` itself fails fast, which is exactly the
/// discrimination these tests need: `AwarenessTooLarge` means the guard
/// fired, any other error means the payload got past it.
fn offline_bus() -> PgNotifyBus {
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(200))
        .connect_lazy("postgres://loki:loki@127.0.0.1:1/loki")
        .unwrap();
    PgNotifyBus {
        pool,
        hub: Arc::new(LocalHub::new()),
    }
}

#[tokio::test]
async fn oversized_awareness_is_rejected_before_any_delivery() {
    let bus = offline_bus();
    let doc = DocumentId::new();
    let mut rx = bus.subscribe(doc).await;

    let payload = vec![0xAB; MAX_AWARENESS_BYTES + 1];
    let err = bus
        .publish_awareness(origin(OURS, 1), doc, &payload)
        .await
        .unwrap_err();

    assert!(
        matches!(err, BusError::AwarenessTooLarge(n) if n == MAX_AWARENESS_BYTES + 1),
        "expected AwarenessTooLarge({}), got {err:?}",
        MAX_AWARENESS_BYTES + 1
    );
    assert!(
        rx.try_recv().is_err(),
        "a rejected awareness payload must not reach local subscribers either"
    );
}

/// The guard's inversion: at exactly the limit it must be **false**, so the
/// payload reaches local subscribers and only the (deliberately unreachable)
/// `NOTIFY` fails.
#[tokio::test]
async fn awareness_at_the_limit_passes_the_guard_and_is_delivered_locally() {
    let bus = offline_bus();
    let doc = DocumentId::new();
    let mut rx = bus.subscribe(doc).await;

    let payload = vec![0xAB; MAX_AWARENESS_BYTES];
    let err = bus
        .publish_awareness(origin(OURS, 1), doc, &payload)
        .await
        .unwrap_err();

    assert!(
        matches!(err, BusError::Transport(_)),
        "at the limit the size guard must not fire; got {err:?}"
    );
    let event = rx.try_recv().unwrap();
    assert_eq!(event.frame, CollabFrame::Awareness(payload));
    assert_eq!(event.origin, origin(OURS, 1));
}

// ---------------------------------------------------------------------------
// Fan-in: one received payload → the event it should produce
// ---------------------------------------------------------------------------

fn update_envelope(instance: Uuid, conn: u64, doc: DocumentId, seq: i64) -> String {
    serde_json::to_string(&Notification {
        instance,
        conn,
        doc: doc.as_uuid(),
        body: Body::Update { seq },
    })
    .unwrap()
}

fn awareness_envelope(instance: Uuid, doc: DocumentId, data: &str) -> String {
    serde_json::to_string(&Notification {
        instance,
        conn: 5,
        doc: doc.as_uuid(),
        body: Body::Awareness {
            data: data.to_owned(),
        },
    })
    .unwrap()
}

#[tokio::test]
async fn foreign_update_is_re_read_from_the_oplog() {
    let doc = DocumentId::new();
    let stores = Arc::new(MemoryStores::new());
    let seq = stores
        .append(doc, UserId::new(), b"loro-update")
        .await
        .unwrap();

    let event = event_from_payload(&update_envelope(THEIRS, 9, doc, seq), stores.as_ref(), OURS)
        .await
        .expect("a foreign update with a live oplog entry must be delivered");

    assert_eq!(event.doc, doc);
    assert_eq!(event.origin, origin(THEIRS, 9));
    // The payload comes from the oplog, not the envelope — the envelope only
    // carried the pointer.
    assert_eq!(event.frame, CollabFrame::Update(b"loro-update".to_vec()));
}

/// Same envelope, same live oplog entry — only the instance id differs. The
/// sole reason for the drop is the self-echo skip.
#[tokio::test]
async fn our_own_notify_echo_is_skipped() {
    let doc = DocumentId::new();
    let stores = Arc::new(MemoryStores::new());
    let seq = stores
        .append(doc, UserId::new(), b"loro-update")
        .await
        .unwrap();

    assert!(
        event_from_payload(&update_envelope(OURS, 9, doc, seq), stores.as_ref(), OURS)
            .await
            .is_none()
    );
}

/// ADR-C013 recovery path: the update was compacted away before we re-read
/// it. Dropping it is correct (subscribers resync from the snapshot); killing
/// the fan-in task or delivering an empty frame would not be.
#[tokio::test]
async fn update_compacted_before_re_read_is_dropped() {
    let doc = DocumentId::new();
    let stores = Arc::new(MemoryStores::new());
    let seq = stores
        .append(doc, UserId::new(), b"loro-update")
        .await
        .unwrap();
    let envelope = update_envelope(THEIRS, 9, doc, seq);

    // Reachable before compaction …
    assert!(
        event_from_payload(&envelope, stores.as_ref(), OURS)
            .await
            .is_some()
    );
    // … and dropped after it.
    stores.truncate_up_to(doc, seq).await.unwrap();
    assert!(
        event_from_payload(&envelope, stores.as_ref(), OURS)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn awareness_is_decoded_and_undecodable_awareness_is_tolerated() {
    let doc = DocumentId::new();
    let stores = Arc::new(MemoryStores::new());

    let good = awareness_envelope(THEIRS, doc, &BASE64.encode(b"cursor"));
    let event = event_from_payload(&good, stores.as_ref(), OURS)
        .await
        .expect("valid base64 awareness must be delivered");
    assert_eq!(event.frame, CollabFrame::Awareness(b"cursor".to_vec()));

    let bad = awareness_envelope(THEIRS, doc, "not base64!!");
    assert!(
        event_from_payload(&bad, stores.as_ref(), OURS)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn malformed_envelopes_are_tolerated() {
    let stores = Arc::new(MemoryStores::new());
    for payload in [
        "",
        "not json",
        r#"{"instance":"not-a-uuid","conn":1,"doc":"00000000-0000-0000-0000-000000000003","kind":"update","seq":1}"#,
        r#"{"conn":1,"kind":"update","seq":1}"#,
    ] {
        assert!(
            event_from_payload(payload, stores.as_ref(), OURS)
                .await
                .is_none(),
            "{payload}"
        );
    }
}
