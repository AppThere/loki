// SPDX-License-Identifier: Apache-2.0

//! The role x route denial matrix, the cross-workspace creation probe, and
//! `Authorization`-header parsing (audit findings F-AP-1, F-AP-2, F-AP-3).
//!
//! `api_flow.rs` walks the happy paths; this file is deliberately one-sided
//! in the other direction — every guarded route is hit as each role *and* as
//! a non-member, and the exact status is pinned from a hand-written table.
//! A regression such as "Editor can manage members" has to fail here.

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{
    create_document, create_workspace, grant_role, send, send_bytes, send_raw_auth, test_router,
    user_id,
};
use serde_json::json;

/// The five callers in every matrix row, in column order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subject {
    /// The document's creator (Owner by construction).
    Owner,
    Editor,
    Commenter,
    Viewer,
    /// Authenticated, but never granted a role on the document.
    NonMember,
}

/// Column order of every `[StatusCode; SUBJECTS]` table below.
const SUBJECTS: [Subject; 5] = [
    Subject::Owner,
    Subject::Editor,
    Subject::Commenter,
    Subject::Viewer,
    Subject::NonMember,
];

impl Subject {
    /// The bearer token (which is also the OIDC subject, so it is also the
    /// identity) used for this caller.
    ///
    /// Exhaustive `match`: a new `Subject` variant cannot compile without
    /// being given a token *and* a row position in [`SUBJECTS`].
    const fn token(self) -> &'static str {
        match self {
            Self::Owner => "alice",
            Self::Editor => "eddie",
            Self::Commenter => "carla",
            Self::Viewer => "vic",
            Self::NonMember => "mallory",
        }
    }

    /// The role granted before the probe, if any.
    const fn grant(self) -> Option<&'static str> {
        match self {
            Self::Owner | Self::NonMember => None,
            Self::Editor => Some("editor"),
            Self::Commenter => Some("commenter"),
            Self::Viewer => Some("viewer"),
        }
    }
}

/// A freshly built server holding one Tier-0 document owned by `alice`, with
/// `subject` granted its role, plus content the read routes need (a snapshot
/// at seq 3 and one attachment).
///
/// One fixture per probe: the snapshot pointer is move-forward-only and role
/// grants are persistent, so sharing a server between cases would let one
/// probe's side effects decide another's status.
struct Fixture {
    app: Router,
    ws_id: String,
    doc_id: String,
    blob_id: String,
    /// The subject's bearer token.
    token: &'static str,
}

async fn fixture(subject: Subject) -> Fixture {
    let app = test_router();
    let ws_id = create_workspace(&app, "alice", "Docs").await;
    let doc_id = create_document(&app, "alice", &ws_id, "Q3 report").await;

    // Seed the content the read probes need, as the owner.
    let (status, _) = send_bytes(
        &app,
        "PUT",
        &format!("/v1/documents/{doc_id}/snapshot?up_to=3"),
        "alice",
        b"snapshot-bytes",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "fixture snapshot upload");
    let (status, created) = send_bytes(
        &app,
        "POST",
        &format!("/v1/documents/{doc_id}/blobs"),
        "alice",
        b"attachment-bytes",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "fixture attachment upload");
    let key = created["key"].as_str().unwrap().to_owned();
    let blob_id = key.rsplit('/').next().unwrap().to_owned();

    if let Some(role) = subject.grant() {
        grant_role(&app, "alice", &doc_id, subject.token(), role).await;
    } else if subject == Subject::NonMember {
        // Provision the account so "non-member" means "known user without a
        // grant", not "unknown user" — otherwise the 404 could come from the
        // user lookup rather than the membership check.
        user_id(&app, subject.token()).await;
    }

    Fixture {
        app,
        ws_id,
        doc_id,
        blob_id,
        token: subject.token(),
    }
}

/// Runs `probe` once per subject against its own fixture and asserts the
/// status against `expected` (in [`SUBJECTS`] order).
async fn assert_matrix_row<F, Fut>(route: &str, expected: [StatusCode; 5], probe: F)
where
    F: Fn(Fixture) -> Fut,
    Fut: std::future::Future<Output = StatusCode>,
{
    for (subject, want) in SUBJECTS.into_iter().zip(expected) {
        let got = probe(fixture(subject).await).await;
        assert_eq!(got, want, "{route} as {subject:?}");
    }
}

// ---------------------------------------------------------------------------
// F-AP-2 — role x route denial matrix
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_document_metadata_matrix() {
    // Action::ReadMetadata — permitted by every role; non-members get 404
    // rather than 403 (membership is not an existence oracle).
    assert_matrix_row(
        "GET /v1/documents/{doc}",
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::NOT_FOUND,
        ],
        |f| async move {
            send(
                &f.app,
                "GET",
                &format!("/v1/documents/{}", f.doc_id),
                Some(f.token),
                None,
            )
            .await
            .0
        },
    )
    .await;
}

#[tokio::test]
async fn get_snapshot_matrix() {
    // Action::ReadContent — every role reads; a non-member cannot tell a
    // private document from a missing one.
    assert_matrix_row(
        "GET /v1/documents/{doc}/snapshot",
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::NOT_FOUND,
        ],
        |f| async move {
            send(
                &f.app,
                "GET",
                &format!("/v1/documents/{}/snapshot", f.doc_id),
                Some(f.token),
                None,
            )
            .await
            .0
        },
    )
    .await;
}

#[tokio::test]
async fn put_snapshot_matrix() {
    // Action::WriteContent — Commenter and Viewer are denied.
    assert_matrix_row(
        "PUT /v1/documents/{doc}/snapshot",
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
        ],
        |f| async move {
            // up_to=9 is ahead of the fixture's seq 3, so a rejection can
            // only come from RBAC, never from the forward-only guard.
            send_bytes(
                &f.app,
                "PUT",
                &format!("/v1/documents/{}/snapshot?up_to=9", f.doc_id),
                f.token,
                b"newer-snapshot",
            )
            .await
            .0
        },
    )
    .await;
}

#[tokio::test]
async fn add_member_matrix() {
    // Action::ManageMembers — Owner only. The Editor cell is the one that
    // matters most: paired with the loki-model rights-matrix gap (F-MO-1),
    // an "Editor can manage members" regression previously passed the whole
    // workspace suite.
    assert_matrix_row(
        "POST /v1/documents/{doc}/members",
        [
            StatusCode::CREATED,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
        ],
        |f| async move {
            // The grantee exists, so a 422 ("user does not exist") cannot be
            // mistaken for a denial.
            let dana = user_id(&f.app, "dana").await;
            send(
                &f.app,
                "POST",
                &format!("/v1/documents/{}/members", f.doc_id),
                Some(f.token),
                Some(json!({ "user_id": dana, "role": "viewer" })),
            )
            .await
            .0
        },
    )
    .await;
}

#[tokio::test]
async fn upload_blob_matrix() {
    // Action::WriteContent — includes the untested Viewer and non-member
    // attachment-upload cells.
    assert_matrix_row(
        "POST /v1/documents/{doc}/blobs",
        [
            StatusCode::CREATED,
            StatusCode::CREATED,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
        ],
        |f| async move {
            send_bytes(
                &f.app,
                "POST",
                &format!("/v1/documents/{}/blobs", f.doc_id),
                f.token,
                b"payload",
            )
            .await
            .0
        },
    )
    .await;
}

#[tokio::test]
async fn download_blob_matrix() {
    // Action::ReadContent — the non-member cell is the one that matters: the
    // object key is guessable from the document id, so the membership check
    // is the only thing standing between a stranger and the attachment.
    assert_matrix_row(
        "GET /v1/documents/{doc}/blobs/{blob}",
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::NOT_FOUND,
        ],
        |f| async move {
            send(
                &f.app,
                "GET",
                &format!("/v1/documents/{}/blobs/{}", f.doc_id, f.blob_id),
                Some(f.token),
                None,
            )
            .await
            .0
        },
    )
    .await;
}

#[tokio::test]
async fn export_matrix() {
    // Action::ReadContent, then the honest 501 for a Tier-0 document: every
    // member gets as far as the unimplemented worker queue, the non-member
    // is stopped at the membership check.
    assert_matrix_row(
        "POST /v1/documents/{doc}/export",
        [
            StatusCode::NOT_IMPLEMENTED,
            StatusCode::NOT_IMPLEMENTED,
            StatusCode::NOT_IMPLEMENTED,
            StatusCode::NOT_IMPLEMENTED,
            StatusCode::NOT_FOUND,
        ],
        |f| async move {
            send(
                &f.app,
                "POST",
                &format!("/v1/documents/{}/export", f.doc_id),
                Some(f.token),
                None,
            )
            .await
            .0
        },
    )
    .await;
}

#[tokio::test]
async fn document_listing_is_filtered_to_members() {
    // The listing has no membership guard of its own — it is filtered
    // per-document — so the interesting assertion is the body, not the
    // status: a non-member gets 200 with an *empty* array.
    for subject in SUBJECTS {
        let f = fixture(subject).await;
        let (status, listing) = send(
            &f.app,
            "GET",
            &format!("/v1/workspaces/{}/documents", f.ws_id),
            Some(f.token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "listing as {subject:?}");
        let want = usize::from(subject != Subject::NonMember);
        assert_eq!(
            listing.as_array().unwrap().len(),
            want,
            "documents visible to {subject:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// F-AP-1 — cross-workspace document creation (audit defect D-6)
// ---------------------------------------------------------------------------

/// **Pins current behaviour, which is not the intended end state.**
///
/// `routes::documents::create` checks only that the workspace *exists*, so any
/// authenticated user can create a document inside someone else's workspace
/// and become its Owner. Workspace-scope membership is a deliberate deferral —
/// `TODO(ws-membership)` in `routes/documents.rs` and
/// `loki-server-collab`/`loki-server-store` — and until it lands this is the
/// actual behaviour, asserted here so it cannot change silently.
///
/// **When `TODO(ws-membership)` lands this test will fail.** That is the
/// point: change the expectation to `NOT_FOUND` (404 keeps workspace
/// existence non-observable, matching the document routes) or `FORBIDDEN`
/// deliberately, rather than discovering afterwards that nothing pinned it.
#[tokio::test]
async fn non_member_can_create_a_document_in_another_users_workspace() {
    let app = test_router();
    let ws_id = create_workspace(&app, "alice", "Alice's private workspace").await;
    let alice_doc = create_document(&app, "alice", &ws_id, "Alice's doc").await;

    let (status, doc) = send(
        &app,
        "POST",
        &format!("/v1/workspaces/{ws_id}/documents"),
        Some("bob"),
        Some(json!({ "title": "Bob was here" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "TODO(ws-membership): creation is currently open to any authenticated \
         caller; tighten this expectation when workspace membership lands"
    );
    let bob_doc = doc["id"].as_str().unwrap().to_owned();

    // What that grant actually buys Bob: Owner rights on his own new document
    // inside Alice's workspace…
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/documents/{bob_doc}"),
        Some("bob"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Bob owns the document he created");

    // …but the blast radius stops at the document scope: the per-document
    // membership check still hides Alice's document from Bob and Bob's from
    // Alice, in both the metadata route and the workspace listing.
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/documents/{alice_doc}"),
        Some("bob"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "Alice's document stays hidden"
    );
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/documents/{bob_doc}"),
        Some("alice"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "Bob's document is his alone");

    for (token, owner) in [("alice", alice_doc.as_str()), ("bob", bob_doc.as_str())] {
        let (_, listing) = send(
            &app,
            "GET",
            &format!("/v1/workspaces/{ws_id}/documents"),
            Some(token),
            None,
        )
        .await;
        let ids: Vec<&str> = listing
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec![owner], "{token} sees only their own document");
    }
}

/// The inverse of the probe above: the existence check that *is* implemented
/// must be false somewhere, or "creation checks the workspace exists" would be
/// a description rather than a guard.
#[tokio::test]
async fn creating_a_document_in_an_unknown_workspace_is_404() {
    let app = test_router();
    let ghost = uuid::Uuid::new_v4();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/v1/workspaces/{ghost}/documents"),
        Some("bob"),
        Some(json!({ "title": "nowhere" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// F-AP-3 — Authorization-header parsing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn authorization_header_must_be_a_bearer_token() {
    let app = test_router();
    let route = "/v1/gdpr/export";

    // Positive control first: the probe can produce a non-401, so the 401s
    // below are the guard speaking and not a broken request builder.
    assert_eq!(
        send_raw_auth(&app, "GET", route, Some(&b"Bearer alice"[..])).await,
        StatusCode::OK,
        "a well-formed bearer token is accepted"
    );

    for (label, raw) in [
        ("no header at all", None),
        ("basic auth", Some(&b"Basic YWxpY2U6cHc="[..])),
        ("lowercase scheme", Some(&b"bearer alice"[..])),
        ("scheme with no token", Some(&b"Bearer"[..])),
        (
            "scheme with no token, trailing space",
            Some(&b"Bearer "[..]),
        ),
        ("token-only, no scheme", Some(&b"alice"[..])),
        // `HeaderValue` accepts arbitrary bytes; `to_str()` is what rejects
        // this, and that branch is otherwise unexercised.
        ("non-UTF-8 header value", Some(&b"Bearer \xff\xfe"[..])),
    ] {
        assert_eq!(
            send_raw_auth(&app, "GET", route, raw).await,
            StatusCode::UNAUTHORIZED,
            "{label} must be rejected"
        );
    }
}
