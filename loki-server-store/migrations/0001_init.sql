-- SPDX-License-Identifier: Apache-2.0
-- Initial schema (LOKI_WEB_SERVER_SPEC.md §4). Forward-only migration.

CREATE TABLE workspace (
    id           UUID PRIMARY KEY,
    name         TEXT NOT NULL,
    default_tier SMALLINT NOT NULL,
    residency    TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE app_user (
    id           UUID PRIMARY KEY,
    oidc_sub     TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    -- X25519 public key for Tier-2 DEK wrapping (ADR-C014).
    public_key   BYTEA
);

CREATE TABLE doc_meta (
    id           UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    tier         SMALLINT NOT NULL,
    residency    TEXT NOT NULL,
    snapshot_ptr TEXT,
    -- WrappedDek JSON: {"algorithm": "...", "blob": "<base64>"} (crypto-agility).
    dek_wrapped  JSONB,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX doc_meta_workspace_idx ON doc_meta (workspace_id, created_at DESC);

CREATE TABLE doc_member (
    doc_id               UUID NOT NULL REFERENCES doc_meta(id) ON DELETE CASCADE,
    user_id              UUID NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
    role                 TEXT NOT NULL,
    -- Tier 2 only: the document DEK wrapped to this member's public key.
    dek_wrapped_for_user JSONB,
    PRIMARY KEY (doc_id, user_id)
);

-- Hot oplog (ADR-C013): stays in Postgres on NVMe, never in object storage.
CREATE TABLE doc_oplog (
    doc_id     UUID NOT NULL REFERENCES doc_meta(id) ON DELETE CASCADE,
    seq        BIGINT GENERATED ALWAYS AS IDENTITY,
    actor      UUID NOT NULL,
    -- Opaque Loro update; AEAD ciphertext under Tier 2.
    payload    BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (doc_id, seq)
);

-- Append-only, hash-chained audit log (ADR-C020). seq is assigned by the
-- application inside an advisory-locked transaction so the chain stays linear.
CREATE TABLE audit_log (
    seq        BIGINT PRIMARY KEY,
    prev_hash  BYTEA NOT NULL,
    hash       BYTEA NOT NULL,
    actor      TEXT NOT NULL,
    action     TEXT NOT NULL,
    target     TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
