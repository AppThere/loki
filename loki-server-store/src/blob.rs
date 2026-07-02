// SPDX-License-Identifier: Apache-2.0

//! Object-storage adapter for snapshots and attachments (ADR-C013/C016).

use std::sync::Arc;

use loki_model::DocumentId;
use object_store::ObjectStore;
use object_store::path::Path;

use crate::error::StoreError;

/// Snapshots and attachments, keyed under the owning document.
///
/// Keys follow the spec layout: `{doc_id}/snap/{version}` for snapshots and
/// `{doc_id}/blob/{blob_id}` for attachments. The backing [`ObjectStore`] is
/// chosen by config URL (Hetzner Object Storage or MinIO — ADR-C016); at-rest
/// encryption (SSE-C or app-layer AEAD) is applied by the caller or the
/// transport, since Hetzner provides none by default.
#[derive(Clone)]
pub struct BlobStore {
    inner: Arc<dyn ObjectStore>,
}

impl BlobStore {
    /// Wraps a configured `object_store` backend.
    #[must_use]
    pub fn new(inner: Arc<dyn ObjectStore>) -> Self {
        Self { inner }
    }

    /// Key of a snapshot version.
    #[must_use]
    pub fn snapshot_key(doc: DocumentId, version: i64) -> String {
        format!("{doc}/snap/{version}")
    }

    /// Key of an attachment.
    #[must_use]
    pub fn attachment_key(doc: DocumentId, blob_id: &str) -> String {
        format!("{doc}/blob/{blob_id}")
    }

    /// Writes a snapshot (Loro `export(Snapshot)` bytes — ciphertext under
    /// Tier 2) and returns its key for `doc_meta.snapshot_ptr`.
    pub async fn put_snapshot(
        &self,
        doc: DocumentId,
        version: i64,
        bytes: Vec<u8>,
    ) -> Result<String, StoreError> {
        let key = Self::snapshot_key(doc, version);
        self.inner
            .put(&Path::from(key.as_str()), bytes.into())
            .await?;
        Ok(key)
    }

    /// Writes an attachment and returns its key.
    pub async fn put_attachment(
        &self,
        doc: DocumentId,
        blob_id: &str,
        bytes: Vec<u8>,
    ) -> Result<String, StoreError> {
        let key = Self::attachment_key(doc, blob_id);
        self.inner
            .put(&Path::from(key.as_str()), bytes.into())
            .await?;
        Ok(key)
    }

    /// Reads an object by key (snapshot pointer or attachment key).
    pub async fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        let result = self.inner.get(&Path::from(key)).await?;
        Ok(result.bytes().await?.to_vec())
    }

    /// Deletes an object by key.
    pub async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.inner.delete(&Path::from(key)).await?;
        Ok(())
    }
}

impl std::fmt::Debug for BlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BlobStore(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

    #[tokio::test]
    async fn snapshot_round_trip() {
        let store = BlobStore::new(Arc::new(InMemory::new()));
        let doc = DocumentId::new();
        let ptr = store
            .put_snapshot(doc, 7, b"snapshot-bytes".to_vec())
            .await
            .unwrap();
        assert_eq!(ptr, format!("{doc}/snap/7"));
        assert_eq!(store.get(&ptr).await.unwrap(), b"snapshot-bytes");
        store.delete(&ptr).await.unwrap();
        assert!(store.get(&ptr).await.is_err());
    }
}
