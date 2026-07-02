// SPDX-License-Identifier: Apache-2.0

//! `loki-server` — the single modular Axum binary (ADR-C012): REST API,
//! WebSocket collaboration relay, and OIDC auth in one artifact, scaled
//! horizontally behind a load balancer or run alone on a small box.

mod config;

use std::sync::Arc;

use jsonwebtoken::DecodingKey;
use loki_crypto::AeadKeyWrap;
use loki_server_api::{ApiState, router};
use loki_server_auth::{OidcVerifier, StaticKeys};
use loki_server_collab::{CollabState, PgNotifyBus};
use loki_server_store::BlobStore;
use loki_server_store::pg::PgStores;
use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::config::{ConfigError, ObjectStoreConfig, ServerConfig};

#[derive(Debug, thiserror::Error)]
enum ServerError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("invalid OIDC verification key: {0}")]
    OidcKey(#[from] jsonwebtoken::errors::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("store error: {0}")]
    Store(#[from] loki_server_store::StoreError),
    #[error("object store error: {0}")]
    ObjectStore(#[from] object_store::Error),
    #[error("collab bus error: {0}")]
    Bus(#[from] loki_server_collab::BusError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = ServerConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(16)
        .connect(&config.database_url)
        .await?;
    let pg = PgStores::new(pool.clone());
    pg.migrate().await?;
    let stores = pg.into_stores();

    let object_store: Arc<dyn ObjectStore> = match &config.object_store {
        ObjectStoreConfig::Memory => {
            tracing::warn!("using in-memory object storage — evaluation only, not durable");
            Arc::new(object_store::memory::InMemory::new())
        }
        ObjectStoreConfig::S3 { bucket } => Arc::new(
            // Credentials/endpoint/region come from the standard AWS_* env
            // vars — works for Hetzner Object Storage and MinIO (ADR-C016).
            AmazonS3Builder::from_env()
                .with_bucket_name(bucket)
                .build()?,
        ),
    };

    // This instance's identity on the fan-out bus (ADR-C012).
    let instance = Uuid::new_v4();
    let bus = PgNotifyBus::start(pool, Arc::clone(&stores.oplog), instance).await?;
    let collab = CollabState::new(Arc::clone(&stores.oplog), bus, instance);

    let verifier = OidcVerifier::new(
        config.oidc_issuer.clone(),
        config.oidc_audience.clone(),
        StaticKeys::single(DecodingKey::from_rsa_pem(&config.oidc_rsa_pem)?),
    );

    let state = ApiState {
        stores,
        blob: BlobStore::new(object_store),
        collab,
        verifier: Arc::new(verifier),
        tier_kek: Arc::new(AeadKeyWrap::new(config.kek)),
        residency: config.residency,
        default_tier: config.default_tier,
    };

    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(bind = %config.bind, %instance, "loki-server listening");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("shutdown complete");
    Ok(())
}

/// Resolves on SIGINT or SIGTERM (systemd/Kubernetes stop).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "ctrl-c handler failed");
        }
    };
    #[cfg(unix)]
    {
        let mut sigterm =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(signal) => signal,
                Err(error) => {
                    tracing::error!(%error, "SIGTERM handler failed; falling back to ctrl-c only");
                    ctrl_c.await;
                    return;
                }
            };
        tokio::select! {
            () = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    ctrl_c.await;
}
