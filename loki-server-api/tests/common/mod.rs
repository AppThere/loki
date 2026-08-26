// SPDX-License-Identifier: Apache-2.0

//! Shared harness for the API integration tests: an in-memory router (no
//! Postgres, no IdP, no object-storage service) plus request helpers.
//!
//! One harness, one derivation — both `api_flow.rs` and `api_rbac.rs` build
//! their router here so the two suites cannot drift into testing differently
//! configured servers.

// Each integration-test binary compiles this module and uses a subset of it.
#![allow(dead_code)]

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
pub struct StubVerifier;

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

pub fn test_router_with_object_store() -> (Router, Arc<dyn object_store::ObjectStore>) {
    let object_store: Arc<dyn object_store::ObjectStore> = Arc::new(InMemory::new());
    let stores = MemoryStores::new().into_stores();
    let collab = CollabState::new(
        Arc::clone(&stores.oplog),
        Arc::new(InMemoryBus::new()),
        Uuid::new_v4(),
    );
    let app = router(ApiState {
        stores,
        blob: BlobStore::new(Arc::clone(&object_store)),
        collab,
        verifier: Arc::new(StubVerifier),
        tier_kek: Arc::new(AeadKeyWrap::new(Kek::generate())),
        residency: Residency::parse("fsn1").unwrap(),
        default_tier: EncryptionTier::TransportAtRest,
    });
    (app, object_store)
}

pub fn test_router() -> Router {
    test_router_with_object_store().0
}

/// Sends a JSON request (or a bodyless one) as `token` and returns the status
/// plus the parsed body (`Value::Null` when the body is not JSON).
pub async fn send(
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

/// Sends a raw-body request (snapshot / attachment upload) and returns
/// status + JSON.
pub async fn send_bytes(
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

/// Sends a request with a verbatim `Authorization` header value (bytes, so a
/// non-UTF-8 value can be exercised). No header is sent when `raw` is `None`.
pub async fn send_raw_auth(
    app: &Router,
    method: &str,
    path: &str,
    raw: Option<&[u8]>,
) -> StatusCode {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(raw) = raw {
        let value = axum::http::HeaderValue::from_bytes(raw).unwrap();
        builder = builder.header(header::AUTHORIZATION, value);
    }
    let request = builder.body(Body::empty()).unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

/// Provisions `token`'s account just-in-time and returns its `user_id`.
pub async fn user_id(app: &Router, token: &str) -> String {
    let (status, body) = send(app, "GET", "/v1/gdpr/export", Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "GDPR export provisions the caller");
    body["user_id"].as_str().unwrap().to_owned()
}

/// Creates a workspace owned by `token` and returns its id.
pub async fn create_workspace(app: &Router, token: &str, name: &str) -> String {
    let (status, ws) = send(
        app,
        "POST",
        "/v1/workspaces",
        Some(token),
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    ws["id"].as_str().unwrap().to_owned()
}

/// Creates a document in `ws` (creator becomes Owner) and returns its id.
pub async fn create_document(app: &Router, token: &str, ws: &str, title: &str) -> String {
    let (status, doc) = send(
        app,
        "POST",
        &format!("/v1/workspaces/{ws}/documents"),
        Some(token),
        Some(json!({ "title": title })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    doc["id"].as_str().unwrap().to_owned()
}

/// Grants `role` on `doc` to the account behind `member_token`, as `owner`.
pub async fn grant_role(
    app: &Router,
    owner: &str,
    doc: &str,
    member_token: &str,
    role: &str,
) -> String {
    let member_id = user_id(app, member_token).await;
    let (status, _) = send(
        app,
        "POST",
        &format!("/v1/documents/{doc}/members"),
        Some(owner),
        Some(json!({ "user_id": member_id, "role": role })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "granting {role}");
    member_id
}
