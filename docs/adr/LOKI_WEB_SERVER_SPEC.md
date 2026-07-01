<!-- SPDX-License-Identifier: Apache-2.0 -->

# AppThere Loki — Web Server & Collaboration Spec

**Status:** Ratified (v1) — open decisions closed 2026-07-01
**Series:** AppThere Cloud, ADRs C012–C020 (extends C009–C011)
**Supersedes:** nothing; extends the AppThere Cloud spec
**Target edition:** Rust 2024

---

## 0. Scope & relationship to existing specs

This spec realises the Loki-specific server: the real-time collaboration relay, the
document/storage API, and the identity and deployment story. It builds directly on the
AppThere Cloud spec (Axum / SQLx / Tokio, OIDC delegation, CRDT relay, ADRs C009–C011)
and does **not** re-litigate those foundations. What is new here:

1. A **tiered encryption model** that reconciles "optional E2EE" with enterprise
   server-side features, resolving the E2EE-vs-server-side-export mutual exclusivity
   already identified in the Cloud spec.
2. First-class **self-hostability**: the same artifact deploys to Hetzner Cloud or to
   the customer's own infrastructure, with no functional gap.
3. An explicit **European Digital Sovereignty** posture aligned to C5:2026, NIS2, and the
   EU Cloud Sovereignty Framework (SEAL / SOV objectives).

The **headless render/print/convert worker** is specified separately in
`LOKI_HEADLESS_SERVER_SPEC.md` (ADRs C021–C027); the two are designed to interlock.

### Goals

- Real-time multi-user editing of Loki documents backed by Loro CRDTs.
- PostgreSQL as the durable system-of-record; object storage for large blobs and snapshots.
- Three selectable confidentiality tiers, including zero-knowledge E2EE.
- One binary, two deployment modes (Hetzner Cloud / self-host), no feature fork.
- Enterprise readiness: SSO, RBAC, audit trail, HA, backup/restore, observability.
- Digital-sovereignty defaults suitable for EU public sector and regulated industry.

### Non-goals (v1)

- Iris raster collaboration (deferred to per-layer ownership per Cloud spec).
- Server-side full-text search over E2EE documents (impossible by construction; see C015).
- Billing (already covered by the Cloud spec's `async-stripe` decision; out of scope here).
- Federation between independent Loki server instances.

### Engineering standards (inherited, non-negotiable)

300-line file ceiling; no `unwrap()`/`expect()` in library code; `#![forbid(unsafe_code)]`
at every crate root; typed errors via `thiserror`; Apache-2.0 SPDX header on line 1;
no hardcoded user-visible strings (`fl!()`); audit-first / implement-second Claude Code
discipline.

---

## 1. Component architecture

```
                         ┌────────────────────────────────────────────┐
                         │              Reverse proxy / LB              │
                         │        (TLS 1.3 termination, HTTP/2)         │
                         └───────────────┬──────────────┬───────────────┘
                                         │              │
                   ┌─────────────────────▼──┐        ┌──▼─────────────────────┐
                   │   loki-server (N≥1)     │  ...   │   loki-server (N≥1)     │
                   │  ── Axum binary ──      │        │                        │
                   │  • REST/API module      │        │                        │
                   │  • WS collaboration mod │        │                        │
                   │  • auth (OIDC) module   │        │                        │
                   └───┬───────────┬─────────┘        └────────────────────────┘
                       │           │  (fan-out bus: pg LISTEN/NOTIFY or Redis)
        ┌──────────────▼──┐    ┌───▼─────────────┐   ┌───────────────────────────┐
        │   PostgreSQL     │    │  Object storage │   │  Job queue (apalis)       │
        │  system-of-record│    │  (S3-compatible)│   │  → headless workers       │
        │  metadata/ACL/   │    │  blobs/snapshots│   │  (render/print/convert)   │
        │  oplog/audit     │    │  SSE-C / app-enc│   └───────────────────────────┘
        └──────────────────┘    └─────────────────┘
                       ▲
        ┌──────────────┴───────────────┐
        │  OIDC IdP (external)          │
        │  Keycloak / Authentik / Zitadel│  ← self-hostable for sovereignty
        └────────────────────────────────┘
```

Crates (all new unless noted):

| Crate | Responsibility |
|---|---|
| `loki-server` | Axum binary; wires modules; config; graceful shutdown. |
| `loki-server-api` | REST surface: workspaces, documents, members, blobs. |
| `loki-server-collab` | WebSocket relay, Loro update ingest, presence/awareness. |
| `loki-server-auth` | OIDC verification, session/token issuance, RBAC checks. |
| `loki-server-store` | Persistence port: Postgres (SQLx) + `object_store` adapter. |
| `loki-crypto` | Envelope encryption, key wrapping, crypto-agility (Tier 1/2). |
| `loki-server-audit` | Append-only, hash-chained audit log. |
| `loki-model` (exists) | Shared `DocumentId`, `WorkspaceId`, ACL types (no server deps). |

The transport port in `loki-server-store` and the fan-out bus in `loki-server-collab` are
traits, so single-node (Postgres LISTEN/NOTIFY) and clustered (Redis pub/sub) deployments
share code — a self-host requirement, not an afterthought.

---

## 2. ADRs

### ADR-C012 — Single modular Axum binary with a pluggable fan-out bus

**Context.** We need horizontal scalability for HA behind a load balancer, but self-hosters
must be able to run everything on one small box. Splitting sync and API into separate
services now would double the operational surface for the 90% single-node case.

**Decision.** Ship one `loki-server` binary containing API, collaboration, and auth as
route modules. Cross-instance awareness (so two collaborators pinned to different instances
see each other) goes through a `FanOutBus` trait with two implementations:
`PgNotifyBus` (default, zero extra infra) and `RedisBus` (for larger clusters). Sticky
sessions at the LB keep a document's WebSocket connections on one instance where possible;
the bus handles the residual cross-instance case.

**Consequences.** One artifact, one config surface. Self-host stays trivial. If a document
becomes a hotspot, we can later shard by `DocumentId` without changing the public API.

---

### ADR-C013 — Loro wire protocol over WebSocket; snapshot + oplog persistence

**Context.** Loki already round-trips full `ParaProps`/`CharProps`/`PageLayout`/`HeaderFooter`
through Loro (per `loro_bridge.rs`). The Cloud spec already committed to Loro's official wire
protocol. The server is a relay, not an authority on document semantics.

**Decision.** Clients exchange Loro update messages over a per-document WebSocket channel.
The server:

- **Relays** updates to all connected members and appends them to a per-document **oplog**.
- **Compacts** periodically into a **snapshot** (Loro `export(Snapshot)`), stored in object
  storage; the oplog is truncated to updates newer than the snapshot.
- **Never merges or interprets** CRDT semantics beyond opaque append + broadcast. This is
  what makes Tier 2 (§ADR-C014) possible: under E2EE the server compacts nothing and merely
  stores encrypted update blobs plus client-produced encrypted snapshots.

**Persistence layout.**

| Store | Data |
|---|---|
| Postgres `doc_oplog` | `(doc_id, seq, actor, payload, created_at)` — recent updates, hot. |
| Object storage | Snapshots (`{doc_id}/snap/{version}`), attachments, images. |
| Postgres `doc_meta` | Title, ACL, current snapshot pointer, encryption tier, residency. |

Object storage is HDD/Ceph on Hetzner and therefore correct for warm snapshots and blobs
but **not** for the hot oplog — the oplog stays in Postgres on NVMe/volume.

**Consequences.** Recovery = latest snapshot + replay of oplog tail. Presence is ephemeral
and never persisted. Awareness (cursors/selections) is broadcast-only.

---

### ADR-C014 — Tiered confidentiality model (Tier 0 / Tier 1 CMK / Tier 2 zero-knowledge)

**Context.** "Optional E2EE" and "enterprise-ready server-side printing/conversion" pull in
opposite directions. A single boolean E2EE flag would either cripple server features or fail
the sovereignty promise. The right primitive is a **tier**, chosen per workspace (default) or
per document (override).

**Decision.** Three tiers, envelope-encrypted throughout via per-document **DEK**s wrapped by
a tier-specific **KEK**:

| Tier | Name | Server sees plaintext? | Server-side render/print/convert/search? | Key custody |
|---|---|---|---|---|
| **0** | Transport + at-rest | Yes | Yes | Server / platform KMS |
| **1** | Customer-managed keys (CMK) | Yes, within trust boundary | Yes | Customer KMS/HSM (Vault, PKCS#11) |
| **2** | Zero-knowledge E2EE | No (ciphertext only) | No | Client only |

- **Tier 0.** TLS 1.3 in transit. At rest: encrypted Postgres volume (LUKS/pgcrypto) and
  **SSE-C or app-layer AEAD** for object storage — mandatory because Hetzner Object Storage
  has **no default at-rest encryption**. Baseline; all features available.
- **Tier 1 (CMK).** Per-document DEK wrapped by a customer-controlled KEK in their KMS/HSM.
  The server decrypts *inside the customer's trust boundary* to run server-side features. Keys
  never leave the customer's jurisdiction. This is the sovereignty sweet spot: confidentiality
  **and** server-side office printing, because in a self-hosted enterprise the server *is* the
  trust boundary.
- **Tier 2 (Zero-knowledge).** DEK is generated and held client-side; the server stores only
  ciphertext. Loro updates are AEAD-encrypted (XChaCha20-Poly1305) client-side before send;
  snapshots are encrypted client-side. Sharing = re-wrapping the DEK to each new member's
  public key (X25519). ACL/membership metadata remains server-visible; content does not.

**Crypto-agility.** All wrapping is expressed through a `KeyWrap` trait so hybrid PQC
(X25519 + ML-KEM) can be adopted without a data migration — C5:2026 calls out post-quantum
readiness explicitly.

**Consequences.** One code path, three policies. Enterprises needing both confidentiality and
server-side processing choose Tier 1. Tier 2 is reserved for maximal-confidentiality documents
that accept the loss of server-side processing.

---

### ADR-C015 — E2EE (Tier 2) and server-side document processing are mutually exclusive

**Context.** Under Tier 2 the server holds only ciphertext, so it cannot render, print,
convert, thumbnail, or index a document. This is a law of the design, not a limitation to be
engineered away.

**Decision.** Make the exclusivity explicit and enforced, not implicit:

- Encryption tier is a first-class field on `doc_meta` and on the workspace default.
- Server-side capabilities (headless print, convert, export, server search, thumbnails) are
  **feature-gated on `tier != 2`**; the API returns a typed `E2eeCapabilityDisabled` error
  rather than silently producing garbage.
- Under Tier 2, those operations are the **client's** responsibility (the client can drive a
  local headless instance — see headless spec ADR-C026).
- The UI must surface the trade-off at the moment a workspace/document is set to Tier 2.

**Consequences.** No surprising failures; the exclusivity is visible in the type system and the
product. A workspace can mix tiers per document if it needs both postures.

---

### ADR-C016 — PostgreSQL system-of-record; object storage via `object_store`

**Context.** The Cloud spec already chose `object_store` for S3 abstraction. We must run
equally on Hetzner Object Storage (managed, EU-only) and MinIO (self-host), with no code fork.

**Decision.**

- **PostgreSQL** holds all relational, transactional, and hot data: workspaces, users, ACLs,
  `doc_meta`, `doc_oplog`, audit log, and the apalis job queue (Postgres backend, so no Redis
  is *required* for jobs — Redis is optional, for the fan-out bus only at scale).
- **Object storage** (via `object_store`) holds snapshots, images, and attachments. Adapter is
  chosen by config URL (`s3://…` for Hetzner/MinIO). SSE-C headers or app-layer AEAD supply the
  at-rest encryption Hetzner does not provide by default.
- Schema migrations via `sqlx migrate`, forward-only, zero-downtime (expand/contract pattern).

**Consequences.** A minimal self-host is Postgres + MinIO + `loki-server`. Managed Hetzner
deployments swap MinIO for Hetzner Object Storage by changing one URL and adding SSE-C keys.

---

### ADR-C017 — OIDC identity delegation; self-hostable IdP; RBAC

**Context.** Sovereignty forbids hard dependence on a US identity provider. Enterprises expect
SSO. The Cloud spec already chose OIDC delegation.

**Decision.**

- The server is an **OIDC relying party**; it never stores passwords. Any compliant IdP works;
  the documented sovereign defaults are **Keycloak / Authentik / Zitadel** (all self-hostable,
  EU-runnable). Azure AD / Google are supported but flagged as non-sovereign.
- Optional **SCIM 2.0** provisioning endpoint for enterprise user lifecycle.
- **RBAC** at workspace and document scope:

  | Role | Rights |
  |---|---|
  | Owner | Full control, membership, deletion, tier changes. |
  | Editor | Read/write content and metadata. |
  | Commenter | Read + comment; no content edit. |
  | Viewer | Read only. |

- Under Tier 2, granting a role additionally requires re-wrapping the document DEK to the new
  member's public key (an out-of-band, client-driven step surfaced by the API).

**Consequences.** No proprietary auth. Enterprises keep identity in-house. Access decisions are
always server-authoritative (metadata), independent of tier.

---

### ADR-C018 — Deployment: Hetzner (OpenTofu) and self-host (Compose / Helm), EU region pinning

**Context.** Users deploy to Hetzner Cloud *or* their own infrastructure. The two must not
diverge functionally.

**Decision.** Three first-class deployment artifacts, all in-repo:

1. **OpenTofu/Terraform module** targeting the `hcloud` provider: CCX (dedicated-vCPU) servers
   for `loki-server` and render workers, Private Networks, a Hetzner Load Balancer, an encrypted
   Volume for Postgres data, and Hetzner Object Storage buckets. **Region is pinned to `fsn1`,
   `nbg1`, or `hel1` (DE/FI) only** — never a US location — with the choice recorded as the
   document residency value.
2. **Docker Compose** (single node): Postgres + MinIO + `loki-server` + one headless worker,
   for evaluation and small self-host.
3. **Helm chart** (k3s/k8s): HA `loki-server` replicas, external or in-cluster Postgres,
   MinIO or external S3, horizontal worker pool.

Config is env + optional file; secrets via env / Docker secrets / Vault. **No phone-home;
telemetry opt-in and off by default.**

**Consequences.** "Deploy to the cloud or your own infrastructure" is literally one of three
supported paths, not a porting exercise.

---

### ADR-C019 — Digital-sovereignty posture

**Context.** Target market is European Digital Sovereignty: EU public sector and regulated
industry. Certifications don't change jurisdiction — a US-owned provider remains exposed to the
US CLOUD Act regardless of badges — so the architecture itself must carry the sovereignty
guarantee.

**Decision.** Sovereignty is a design property, mapped to the EU Cloud Sovereignty Framework's
SOV objectives:

- **Data residency (SOV-2).** All data in EU DCs; residency pinned and recorded per document;
  object storage region enforced at config validation time.
- **No non-EU jurisdiction dependency (SOV-1).** Hetzner is a German GmbH operating EU-only
  DCs; self-hostable IdP; no mandatory US SaaS in the data path.
- **Open standards / no lock-in / portability (SOV-6).** `object_store` abstraction, standard
  S3, standard OIDC, open-source stack, documented backup/restore → workload is portable off
  Hetzner or off the cloud entirely.
- **Operational sovereignty (SOV-4).** Full self-host path means the customer's own staff can
  operate, patch, and recover with no vendor in the loop.
- **Crypto (C5:2026).** Crypto-agile key wrapping ready for hybrid PQC; confidential-computing
  hardening for Tier-1 (AMD SEV-SNP / Intel TDX) is **adopted** — see headless ADR-C028.

Design to **C5:2026 / EUCS-Substantial-level** controls. EUCS itself remains voluntary and its
"High/sovereignty" tier is still unresolved at EU level, so we target the stable C5:2026 baseline
and track the forthcoming BSI cloud-sovereignty catalogue.

**Consequences.** The sovereignty story survives audit because it rests on ownership,
residency, and portability — not on marketing.

---

### ADR-C020 — Audit, retention, and GDPR data-subject operations

**Decision.**

- **Append-only, hash-chained audit log** (`loki-server-audit`): auth events, ACL changes, tier
  changes, exports, deletions. Each entry carries the prior entry's hash → tamper-evident.
- **Retention & legal hold** via object-storage **Object Lock** (retention + legal hold),
  available on Hetzner and MinIO.
- **GDPR operations** as first-class endpoints: data export (portability), right-to-erasure
  (crypto-shredding the document DEK renders Tier 1/2 ciphertext unrecoverable — clean erasure),
  and records of processing. DPA-ready.

**Consequences.** Erasure is provable under Tier 1/2 by destroying keys; audit is defensible.

---

## 3. API surface (sketch)

REST, JSON, versioned under `/v1`. WebSocket for collaboration.

```
POST   /v1/workspaces
GET    /v1/workspaces/{ws}/documents
POST   /v1/workspaces/{ws}/documents            # sets tier (default = workspace tier)
GET    /v1/documents/{doc}                       # metadata; content via WS/snapshot
GET    /v1/documents/{doc}/snapshot              # latest snapshot (cipher or plain by tier)
POST   /v1/documents/{doc}/members               # role grant (+ DEK re-wrap under Tier 2)
POST   /v1/documents/{doc}/blobs                 # attachment/image upload
WS     /v1/documents/{doc}/collab                # Loro updates + awareness
POST   /v1/documents/{doc}/export                # 409 E2eeCapabilityDisabled if tier==2
GET    /v1/gdpr/export        POST /v1/gdpr/erase
```

Errors are typed (`thiserror`) and mapped to stable problem+json codes; `E2eeCapabilityDisabled`
is the canonical Tier-2/server-side rejection.

---

## 4. Data model (essentials)

```sql
workspace(id, name, default_tier, residency, created_at)
app_user(id, oidc_sub, display_name, public_key /* X25519, for Tier 2 */)
doc_meta(id, workspace_id, title, tier, residency, snapshot_ptr, dek_wrapped, created_at)
doc_member(doc_id, user_id, role, dek_wrapped_for_user /* Tier 2 only */)
doc_oplog(doc_id, seq, actor, payload, created_at)     -- payload is ciphertext under Tier 2
audit_log(id, prev_hash, hash, actor, action, target, created_at)
-- apalis job tables (Postgres backend)
```

---

## 5. HA, backup, observability

- **HA:** ≥2 `loki-server` replicas behind the LB; Postgres primary + streaming replica;
  fan-out bus makes collaboration instance-independent.
- **Backup:** Postgres PITR (WAL archived to object storage); object storage versioning +
  Object Lock; documented restore runbook.
- **Observability:** OpenTelemetry traces, Prometheus metrics, structured JSON logs. No content
  or plaintext ever logged; under Tier 2 nothing to leak by construction.
- **Security headers, rate limiting, CSP** at the proxy and app layers.

---

## 6. Ratified decisions

1. **Default tier — RATIFIED.** Tier 0 default for SaaS; Tier 1 default for enterprise
   self-host; Tier 2 opt-in per document.
2. **Fan-out bus — RATIFIED.** `PgNotifyBus` is the default. Switch to `RedisBus` when any of:
   `loki-server` replicas > 3; sustained cross-instance update fan-out > ~1,000 msg/s; or
   concurrent collaborative WebSocket connections > ~2,000. The switch point is a single config
   tunable; these are shipped defaults, not hard limits.
3. **Presence under Tier 2 — RATIFIED.** Awareness payloads are AEAD-encrypted client-side —
   cursor sharing is retained, contents opaque to the server.
4. **ADR numbering — RATIFIED.** The C-series continues at C012.
