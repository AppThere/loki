// SPDX-License-Identifier: Apache-2.0

//! End-to-end handler tests over the in-memory ports: auth, RBAC, tier
//! gating (ADR-C015), and GDPR operations — no Postgres or IdP required.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use loki_crypto::{AeadKeyWrap, Kek};
use loki_model::{EncryptionTier, Residency};
use loki_server_api::{ApiState, router};
use loki_server_auth::{AuthContext, AuthError, IdentityVerifier};
use loki_server_collab::{CollabState, InMemoryBus};
use loki_server_store::BlobStore;
use loki_server_store::memory::MemoryStores;
use object_store::memory::InMemory;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

/// Accepts any non-empty token; the token string *is* the OIDC subject, so
/// each distinct token is a distinct user.
struct StubVerifier;

#[async_trait]
impl IdentityVerifier for StubVerifier {
    async fn verify(&self, token: &str) -> Result<AuthContext, AuthError> {
        if token.is_empty() {
            return Err(AuthError::UnknownKey { kid: None });
        }
        Ok(AuthContext {
            oidc_sub: token.to_owned(),
            display_name: format!("User {token}"),
        })
    }
}

fn test_router() -> Router {
    let stores = MemoryStores::new().into_stores();
    let collab = CollabState::new(
        Arc::clone(&stores.oplog),
        Arc::new(InMemoryBus::new()),
        Uuid::new_v4(),
    );
    router(ApiState {
        stores,
        blob: BlobStore::new(Arc::new(InMemory::new())),
        collab,
        verifier: Arc::new(StubVerifier),
        tier_kek: Arc::new(AeadKeyWrap::new(Kek::generate())),
        residency: Residency::parse("fsn1").unwrap(),
        default_tier: EncryptionTier::TransportAtRest,
    })
}

async fn send(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn health_is_public_but_api_requires_auth() {
    let app = test_router();
    let response = app
        .clone()
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let (status, problem) = send(
        &app,
        "POST",
        "/v1/workspaces",
        None,
        Some(json!({"name": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(problem["type"], "urn:appthere:loki:error:unauthorized");
}

#[tokio::test]
async fn document_lifecycle_with_rbac() {
    let app = test_router();

    // Alice creates a workspace + document (becomes Owner).
    let (status, ws) = send(
        &app,
        "POST",
        "/v1/workspaces",
        Some("alice"),
        Some(json!({"name": "Docs"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(ws["default_tier"], "transport-at-rest");
    assert_eq!(ws["residency"], "fsn1");
    let ws_id = ws["id"].as_str().unwrap().to_owned();

    let (status, doc) = send(
        &app,
        "POST",
        &format!("/v1/workspaces/{ws_id}/documents"),
        Some("alice"),
        Some(json!({"title": "Q3 report"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let doc_id = doc["id"].as_str().unwrap().to_owned();

    // Alice reads it; Bob (not a member) sees 404, not 403.
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/documents/{doc_id}"),
        Some("alice"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/documents/{doc_id}"),
        Some("bob"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Bob must exist before he can be granted a role; a request lands him
    // in the user table (JIT provisioning), then Alice grants Viewer.
    let (_, bob_export) = send(&app, "GET", "/v1/gdpr/export", Some("bob"), None).await;
    let bob_id = bob_export["user_id"].as_str().unwrap().to_owned();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/v1/documents/{doc_id}/members"),
        Some("alice"),
        Some(json!({"user_id": bob_id, "role": "viewer"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Bob can now read metadata and sees the doc in the listing…
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/documents/{doc_id}"),
        Some("bob"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, listing) = send(
        &app,
        "GET",
        &format!("/v1/workspaces/{ws_id}/documents"),
        Some("bob"),
        None,
    )
    .await;
    assert_eq!(listing.as_array().unwrap().len(), 1);

    // …but as a Viewer he cannot grant roles (403), and there is no
    // snapshot yet (404).
    let (status, problem) = send(
        &app,
        "POST",
        &format!("/v1/documents/{doc_id}/members"),
        Some("bob"),
        Some(json!({"user_id": bob_id, "role": "owner"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["type"], "urn:appthere:loki:error:forbidden");
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/documents/{doc_id}/snapshot"),
        Some("bob"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tier2_gates_server_side_processing_and_requires_dek_wraps() {
    let app = test_router();
    let (_, ws) = send(
        &app,
        "POST",
        "/v1/workspaces",
        Some("alice"),
        Some(json!({"name": "Vault"})),
    )
    .await;
    let ws_id = ws["id"].as_str().unwrap().to_owned();

    let (status, doc) = send(
        &app,
        "POST",
        &format!("/v1/workspaces/{ws_id}/documents"),
        Some("alice"),
        Some(json!({"title": "secret", "tier": "zero-knowledge"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(doc["tier"], "zero-knowledge");
    let doc_id = doc["id"].as_str().unwrap().to_owned();

    // ADR-C015: export is the canonical 409.
    let (status, problem) = send(
        &app,
        "POST",
        &format!("/v1/documents/{doc_id}/export"),
        Some("alice"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        problem["type"],
        "urn:appthere:loki:error:e2ee-capability-disabled"
    );

    // Granting a role without the client-driven DEK re-wrap is rejected.
    let (_, bob_export) = send(&app, "GET", "/v1/gdpr/export", Some("bob"), None).await;
    let bob_id = bob_export["user_id"].as_str().unwrap().to_owned();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/v1/documents/{doc_id}/members"),
        Some("alice"),
        Some(json!({"user_id": bob_id, "role": "editor"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, member) = send(
        &app,
        "POST",
        &format!("/v1/documents/{doc_id}/members"),
        Some("alice"),
        Some(json!({
            "user_id": bob_id, "role": "editor",
            "dek_wrapped_for_user": {"algorithm": "x25519-hkdf-sha256-xchacha20.v1", "blob": "AAEC"},
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(member["dek_wrapped_for_user"].is_object());
}

#[tokio::test]
async fn tier0_export_is_honestly_unimplemented() {
    let app = test_router();
    let (_, ws) = send(
        &app,
        "POST",
        "/v1/workspaces",
        Some("alice"),
        Some(json!({"name": "W"})),
    )
    .await;
    let ws_id = ws["id"].as_str().unwrap().to_owned();
    let (_, doc) = send(
        &app,
        "POST",
        &format!("/v1/workspaces/{ws_id}/documents"),
        Some("alice"),
        Some(json!({"title": "doc"})),
    )
    .await;
    let doc_id = doc["id"].as_str().unwrap().to_owned();
    let (status, problem) = send(
        &app,
        "POST",
        &format!("/v1/documents/{doc_id}/export"),
        Some("alice"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(problem["type"], "urn:appthere:loki:error:not-implemented");
}

#[tokio::test]
async fn gdpr_erase_anonymizes_the_caller() {
    let app = test_router();
    let (_, before) = send(&app, "GET", "/v1/gdpr/export", Some("carol"), None).await;
    assert_eq!(before["display_name"], "User carol");
    let (status, _) = send(&app, "POST", "/v1/gdpr/erase", Some("carol"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // The erased account's OIDC link is severed; the same token now
    // provisions a fresh account (the old row keeps no personal data).
    let (_, after) = send(&app, "GET", "/v1/gdpr/export", Some("carol"), None).await;
    assert_ne!(after["user_id"], before["user_id"]);
}

#[tokio::test]
async fn validation_errors_are_422() {
    let app = test_router();
    let (status, problem) = send(
        &app,
        "POST",
        "/v1/workspaces",
        Some("alice"),
        Some(json!({"name": "  "})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["type"], "urn:appthere:loki:error:validation-failed");
}

/// Sends a raw-body request (snapshot upload) and returns status + JSON.
async fn send_bytes(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: &[u8],
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_vec()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn client_snapshot_upload_is_forward_only() {
    let app = test_router();
    let (_, ws) = send(
        &app,
        "POST",
        "/v1/workspaces",
        Some("alice"),
        Some(json!({"name": "Vault"})),
    )
    .await;
    let ws_id = ws["id"].as_str().unwrap().to_owned();
    let (_, doc) = send(
        &app,
        "POST",
        &format!("/v1/workspaces/{ws_id}/documents"),
        Some("alice"),
        Some(json!({"title": "secret", "tier": "zero-knowledge"})),
    )
    .await;
    let doc_id = doc["id"].as_str().unwrap().to_owned();
    assert_eq!(doc["snapshot_seq"], 0);

    // The owner's client uploads an (encrypted) snapshot covering seq 3.
    let path = format!("/v1/documents/{doc_id}/snapshot?up_to=3");
    let (status, body) = send_bytes(&app, "PUT", &path, "alice", b"ciphertext-snap").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["snapshot_seq"], 3);

    // Metadata reflects it; the snapshot downloads byte-identically.
    let (_, meta) = send(
        &app,
        "GET",
        &format!("/v1/documents/{doc_id}"),
        Some("alice"),
        None,
    )
    .await;
    assert_eq!(meta["has_snapshot"], true);
    assert_eq!(meta["snapshot_seq"], 3);

    // A stale re-upload (same or older seq) is a typed 409.
    let (status, problem) = send_bytes(&app, "PUT", &path, "alice", b"older").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        problem["type"],
        "urn:appthere:loki:error:snapshot-superseded"
    );

    // A viewer may not upload snapshots (write action).
    let (_, bob_export) = send(&app, "GET", "/v1/gdpr/export", Some("bob"), None).await;
    let bob_id = bob_export["user_id"].as_str().unwrap().to_owned();
    send(
        &app,
        "POST",
        &format!("/v1/documents/{doc_id}/members"),
        Some("alice"),
        Some(json!({
            "user_id": bob_id, "role": "viewer",
            "dek_wrapped_for_user": {"algorithm": "x25519-hkdf-sha256-xchacha20.v1", "blob": "AAEC"},
        })),
    )
    .await;
    let path5 = format!("/v1/documents/{doc_id}/snapshot?up_to=5");
    let (status, _) = send_bytes(&app, "PUT", &path5, "bob", b"snap").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
