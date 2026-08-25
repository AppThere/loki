# Test-efficacy audit — server & crypto crates

Scope: loki-server-api, loki-server-audit, loki-server-auth, loki-server-collab,
loki-server-store, loki-server, loki-model, loki-crypto.
Method: every test file read in full; for each suspicious test the production code
it calls was read and a mutation thought-experiment applied ("would this test fail
if the guard were inverted / the function stubbed?"). Static analysis only.

Categories: A = cannot fail, B = doesn't test what it claims, C = material gap.

---

## loki-crypto (13 tests: dek.rs 4, aead_wrap.rs 3, wrap.rs 1, x25519_wrap_tests.rs 5)

### F-CR-1 — C, MEDIUM — no known-answer vector anywhere; all crypto tests are self-referential round-trips
- Files: `loki-crypto/src/dek.rs:103-136`, `loki-crypto/src/x25519_wrap_tests.rs` (whole file)
- Every encrypt/decrypt and wrap/unwrap test uses the *same implementation* for both
  directions. The negative cases (wrong key, wrong AAD, tamper) prove authentication
  works, but nothing pins the wire format: `nonce(24) || ct+tag`, the ephemeral-pk
  prefix layout in `x25519_wrap.rs:138-140`, or the HKDF salt/info construction in
  `derive_wrapping_key` (`x25519_wrap.rs:110-124`).
- Why it matters here specifically: Tier-2 unwrap happens **on clients**
  (`x25519_wrap.rs` doc: "unwrapping happens on clients holding the member's secret
  key"), which may be independently implemented (web). A silent change to salt order
  or blob layout passes the entire suite while breaking every existing wrapped DEK
  and all cross-implementation interop.
- Fixed test: decrypt a hard-coded blob (fixed key, fixed nonce injected or a
  once-generated pinned fixture) and assert the exact plaintext; assert a pinned
  hex wrapping-key for a fixed X25519 shared secret + salt.

### F-CR-2 — C, LOW — `wrapped_dek_json_round_trip` doesn't pin the serialized form
- File: `loki-crypto/src/wrap.rs:67-75`
- `let json = serde_json::to_string(&wrapped)...; assert_eq!(back, wrapped);` —
  the base64-STANDARD encoding contract (`serde_bytes_base64`) is stored in
  `doc_meta.dek_wrapped`; switching to hex or URL-safe base64 would still round-trip.
- Fixed test: assert the exact JSON string for a fixed blob (e.g. `"blob":"AAEC..."`).

### F-CR-3 — C, LOW — no nonce-uniqueness / non-determinism assertion
- File: `loki-crypto/src/dek.rs` tests
- `seal` is called at most once per key in any test. An RNG regression producing a
  constant nonce (catastrophic for XChaCha) passes every test.
- Fixed test: seal the same plaintext twice, assert the two blobs differ and their
  first 24 bytes (nonces) differ.

Healthy: wrong-key + wrong-AAD (`dek.rs:111-123`), tag tamper + truncation
(`dek.rs:126-136`), algorithm-tag mismatch (`aead_wrap.rs:111-119`), wrap-only
instance cannot unwrap = zero-knowledge property (`x25519_wrap_tests.rs:24-33`),
wrong-member unwrap fails (`:36-47`), Debug never leaks key material. This is
genuinely two-sided coverage; only the pinning is missing.

---

## loki-server-audit (7 tests: entry_tests.rs 6, action.rs 1)

### F-AU-1 — C, MEDIUM — no known-answer test for `compute_hash`; DB-compat unpinned
- File: `loki-server-audit/src/entry_tests.rs` (whole file), production `entry.rs:80-98`
- All chains are built with `AuditEntry::append` and verified with `verify_chain` —
  the same `compute_hash` on both sides. A change to the `b"loki-audit.v1"` domain
  separator, field order, or timestamp precision passes every test while rendering
  every chain already persisted in `audit_log` unverifiable.
- Fixed test: a fixed entry (fixed fields + timestamp) asserted against a hard-coded
  32-byte hex hash.

### F-AU-2 — C, MEDIUM — the length-prefix forgery defense is untested
- File: production `entry.rs:78-95` ("length-prefixed ... so field boundaries cannot
  be shifted to forge a colliding encoding"); no test in `entry_tests.rs`
- The stated purpose of the length prefixes is collision resistance across field
  boundaries, but no test asserts that (actor="ab", target="c") and (actor="a",
  target="bc")-style shifted encodings hash differently. Removing the length
  prefixes (`hasher.update(field.as_bytes())` only) survives the whole suite.
- Fixed test: two entries whose concatenated fields are byte-identical but split
  differently must have different hashes.

### F-AU-3 — C (context), HIGH — `verify_chain` has no production caller
- `grep verify_chain` across loki-server, loki-server-api, loki-server-store: only
  the trait/`load_chain` exist; no route, startup check, or admin command invokes
  `verify_chain`. The tamper-evidence is exercised only by tests — the running
  system never detects tampering. (Placement issue per CLAUDE.md rule 7, not a test
  defect per se, but it caps the value of the otherwise-good tamper tests.)

Healthy: mutation, removal, replaced-entry (recomputed hash, broken link), genesis,
and timestamp tampering are all detected with exact typed errors — a model suite
for the mechanism itself.

---

## loki-server-auth (11 tests: verifier_tests.rs 5, jwks_tests.rs 5, rbac.rs 1)

### F-AA-1 — C, HIGH — no wrong-signature test anywhere
- Files: `loki-server-auth/src/verifier_tests.rs` (all 5), `jwks_tests.rs` (all 5)
- Every token in the suite is signed with the secret the verifier holds. Negative
  cases cover wrong issuer/audience/expiry (`verifier_tests.rs:61-72`), unknown kid,
  and garbage input — but never a *well-formed token signed with a different key*
  presented to a verifier that has the right key.
- Mutation experiment: if `OidcVerifier::verify` disabled signature validation
  (e.g. `validation.insecure_disable_signature_validation()` or an equivalent
  regression in the jsonwebtoken usage), every existing test still passes: the
  claim-validation failures still fail on claims, and the positive cases still
  succeed. The single most security-critical property of the verifier is untested.
- Fixed test: `encode(..., EncodingKey::from_secret(b"other-secret"))` against the
  standard `verifier()` → must be `Err(AuthError::InvalidToken(_))`.

### F-AA-2 — C, MEDIUM — no algorithm-confusion test
- File: `verifier_tests.rs`; production `verifier.rs:114-117` (`validation.algorithms`)
- No test presents a token with an algorithm outside the allow-list (e.g. HS384 when
  only HS256 is allowed, or `alg=none`). The `with_algorithms` allow-list is the
  guard that keeps the production RS256/ES256 pin meaningful; it is never false in
  any test.
- Fixed test: HS384-signed token against an HS256-only verifier → rejected.

### F-AA-3 — C, MEDIUM — `StaticKeys` serves the default key for *any* unknown kid; behavior unpinned
- Production: `verifier.rs:52` — `self.keys.get(kid).or(self.default_key.as_ref())`
- A token with a forged `kid` is verified against the default key rather than being
  rejected. Whether that fallback is intended is undocumented, and no test pins it
  either way (the `unknown_kid_is_rejected` test uses a source with *no* default
  key, so the fallback branch is never exercised).
- Fixed test: `StaticKeys::single(k)` + token with `kid: "forged"` — assert the
  intended outcome explicitly (accept-by-fallback or reject), so a change is caught.

### F-AA-4 — C, LOW — rotation removes old keys, but "old kid stops verifying after rotation" is untested
- File: `jwks_tests.rs:104-115` — `rotation_refetches_on_unknown_kid` proves the new
  key works; it never asserts that a k1-signed token now fails (cache `rebuild`
  clears old keys, `jwks.rs:90`). A rebuild that *merged* instead of replacing —
  keeping compromised rotated-out keys alive — would pass.

Healthy: rotation-on-unknown-kid, refetch throttling with call counting (a real
DoS-guard inversion test), fetch-failure degradation, kidless single-key serving,
and `rbac.rs` distinguishing NotMember from Forbidden with an allowed case — all
two-sided.

---

## loki-model (10 tests: role.rs 2, ids.rs 3, residency.rs 3, tier.rs 2)

### F-MO-1 — C, HIGH — rights-matrix test covers ~13 of 32 role×action cells; `Editor.allows(Delete)` mutation survives
- File: `loki-model/src/role.rs:97-114`; production `role.rs:48-59`
- The test never asserts `!Editor.allows(Action::Delete)`, `!Commenter.allows(WriteMetadata)`,
  `!Viewer.allows(WriteMetadata / ManageMembers / ChangeTier / Delete)`,
  `!Commenter.allows(ManageMembers / ChangeTier / Delete)`, nor the positive cells
  `Commenter.allows(ReadContent)`, `Editor.allows(Comment)`, `Owner.allows(WriteContent/Comment)`.
- Concrete surviving mutation (verified against the match arms): move `Action::Delete`
  from the Owner-only arm into the `WriteContent | WriteMetadata` arm — Editors can
  now delete documents, and no test in the workspace fails (the API integration
  tests never exercise a delete route; `TODO(server-tier-delete)` in
  `loki-server-store/src/ports.rs:75-80` confirms no route exists). This is the
  RBAC ground truth for the whole server; it deserves the full table.
- Fixed test: exhaustive 4×8 nested loop against a literal expected table.

### F-MO-2 — A, LOW — `ids_are_distinct_types` asserts a tautology
- File: `loki-model/src/ids.rs:100-105`
- `let ws = WorkspaceId::from_uuid(doc.as_uuid()); assert_eq!(doc.as_uuid(), ws.as_uuid());`
  — true by construction; the runtime assertion cannot fail regardless of any code
  change. (The comment admits the property is compile-time; the assertion is
  decoration.) Harmless, but it inflates the count of "tests".

### F-MO-3 — B, LOW — `roles_order_by_privilege` tests parse round-trips, not ordering
- File: `role.rs:117-125`
- Name and comment claim privilege ordering; the body asserts `as_str().parse()`
  round-trips and rejects "admin". No ordering property is asserted (Ord is
  deliberately not derived — then the test should be named for what it does).

Healthy: residency EU-pin two-sided (accepted regions + `ash`/`us-east-1` rejected
+ empty self-hosted label), tier numeric round-trip incl. out-of-range, tier-2
capability gates asserted false (and true for tiers 0/1 on processing).

---

## loki-server-collab (14 tests: compact_tests.rs 5, relay_tests.rs 4, bus_memory.rs 1, hub.rs 2, msg.rs 2)

### F-CO-1 — B, HIGH — `lost_race_leaves_newer_snapshot_intact` never reaches the LostRace branch
- File: `loki-server-collab/src/compact_tests.rs:189-207`; production `compact.rs:96-124`
- The test advances `snapshot_seq` to 5 *before* calling `compact_document`, so
  `fetch_after(doc, 5)` filters out the seq-1 entry and the pass returns at
  `CompactionOutcome::NothingToDo` (`compact.rs:97-99`) — the test itself asserts
  `NothingToDo`, not `LostRace`. The actual race branch — entries fetched against a
  stale `snapshot_seq`, `set_snapshot` returning `false`, the freshly-written blob
  deleted, `LostRace` returned (`compact.rs:120-123`) — has **zero** coverage.
  Stub that branch (skip the `blob.delete`, or return `Compacted` after losing the
  guard and truncate anyway — the orphaned-truncation bug the guard exists to stop)
  and no test fails.
- Fixed test: interleave — fetch entries via a store wrapper that bumps
  `snapshot_seq` between `fetch_after` and `set_snapshot` (or call `set_snapshot`
  concurrently), then assert `LostRace`, assert the loser's blob was deleted, and
  assert the oplog was NOT truncated.

### F-CO-2 — C, HIGH — `PgNotifyBus` / `fan_in_loop` (the real cross-instance relay) has zero tests
- File: `loki-server-collab/src/bus_pg.rs` (210 lines, no `#[cfg(test)]`)
- Every relay/bus test runs on `InMemoryBus`, which bypasses the interesting part:
  the NOTIFY envelope serde, self-instance echo skip (`bus_pg.rs:123-125`),
  pointer-based update re-read with the compacted-before-read fallback (`:132-144`),
  undecodable-awareness tolerance, and the `MAX_AWARENESS_BYTES` rejection
  (`:188-190` — the only place `BusError::AwarenessTooLarge` is produced, asserted
  nowhere). The envelope logic (Notification serde round-trip, size arithmetic
  vs the 8000-byte NOTIFY cap) is testable without Postgres; none of it is.
- Fixed tests: unit-test `Notification` serde (incl. the flattened `kind` tag),
  oversized awareness → typed error, and factor `fan_in_loop`'s per-message body
  into a function testable with a fake `OplogStore`.

### F-CO-3 — C, MEDIUM — `drive_socket` (ws.rs) untested: close-code contract and backlog-then-live ordering
- File: `loki-server-collab/src/ws.rs` (no tests)
- The relay was factored out "so it is testable without sockets" — and it is — but
  the socket pump's own contract (1008 on write-denied/malformed frame, 1011 on
  internal error, 1012 on lag→resync, backlog replayed before live events, the
  subscribe-before-backlog gap-freedom claim) is asserted nowhere. A regression
  swapping subscribe/backlog order (losing events in the gap) passes the suite.

### F-CO-4 — C, LOW — relay error paths untested; role revocation mid-connection unexercised
- `RelayError::Store`/`Bus` propagation from `ingest` never triggered by any test;
  `wants()` filters only by origin — a member revoked after connect keeps streaming
  (documented trust model in `collab.rs:33-36`, but no test pins it as intended).

Healthy: compaction happy path with *real* Loro documents (content asserted, not
just lengths), corrupt-payload abort without truncation, tier-2 skip with
ciphertext left intact, write-denied for read-only members with oplog asserted
empty (a true denial test), awareness-never-persisted, cross-document isolation,
echo suppression.

---

## loki-server-store (2 tests: memory/mod.rs 1, blob.rs 1)

### F-ST-1 — C, HIGH — the entire Postgres implementation is untested; the memory double's "mirror" is unenforced
- Files: `loki-server-store/src/pg/{audit,document,member,oplog,user,workspace}.rs`
  (~600 lines of SQL) — zero tests of any kind (no sqlx::test, no testcontainers).
- Everything security-relevant that the rest of the workspace "tests" runs on
  `MemoryStores`, whose header claims it "mirrors the Postgres semantics"
  (`memory/mod.rs:5-8`) — but no contract-test suite runs the same assertions
  against both implementations, so the mirror is a comment, not a property. The
  ADR-C013 forward-only guard SQL (`pg/document.rs:77-93`), the advisory-lock audit
  append (`pg/audit.rs:55-87`), and the `(xmax = 0)` provisioning idiom
  (`pg/user.rs:40-48`) are exactly the load-bearing, easy-to-get-wrong parts.
- Fix: a port-contract test suite (a macro/generic fn over `dyn DocumentStore` etc.)
  run against MemoryStores unconditionally and PgStores behind an env-gated
  integration harness.

### F-ST-2 — B, MEDIUM — memory double diverges from Postgres on `upsert_user_by_oidc` display-name refresh
- Memory (`memory/mod.rs:104-105`): existing user returned unchanged — the new
  `display_name` is discarded. Postgres (`pg/user.rs:40`):
  `ON CONFLICT ... DO UPDATE SET display_name = EXCLUDED.display_name` — the name
  follows the IdP. The memory test (`memory/mod.rs:144-162`) even passes a changed
  name ("Ada" → "Ada L.") on the second call but only asserts the id and the bool,
  so the divergence is invisible to the suite. Any API test asserting a display
  name after re-login would pass against memory and fail against Postgres.
- Fixed: align the double (update the name) and assert the returned record's name.

### F-ST-3 — C, LOW — duplicate-key behavior diverges and is untested
- Memory `create_document`/`create_workspace`/`upsert_member` use `HashMap::insert`
  (silent overwrite); Postgres plain `INSERT` raises a unique violation →
  `StoreError`. No test exercises duplicate creation on either impl.

### F-ST-4 — C, LOW (production nit found while tracing) — `pg/user.rs:48`
- `row.try_get("newly_provisioned").unwrap_or(false)` — a decode failure silently
  reports "not newly provisioned", dropping the once-per-account AuthLogin audit
  (ADR-C020). Untestable today because of F-ST-1.

---

## loki-server-api (8 tests, all in tests/api_flow.rs)

### F-AP-1 — C, HIGH — document creation has no workspace authorization, and no test probes the denial
- Production: `src/routes/documents.rs:22-59` — `create` checks only that the
  workspace *exists*; any authenticated user can create documents in any other
  user's workspace (becoming Owner of a doc inside it, and thereby appearing in
  that workspace's listing). Workspace-scope membership is a known deferral
  (`TODO(ws-membership)`), but the test suite contains no "bob creates a document
  in alice's workspace" probe at all, so the current behavior — whether intended
  interim or a live authz gap — is unpinned and a future fix has no failing test
  to turn green. One-sided authorization coverage at its most consequential spot.
- Fixed test: assert the intended status for a non-member's create (403/404 once
  ws-membership lands; explicitly 201-with-comment if the interim openness is
  deliberate).

### F-AP-2 — C, MEDIUM — RBAC denial matrix over routes is thin
- File: `tests/api_flow.rs`
- Tested denials: non-member GET doc → 404 (:152-160), Viewer grants role → 403
  (:198-207), Viewer snapshot PUT → 403 (:433-435). Untested: Commenter/Editor ×
  ManageMembers (the Editor cell matters most — combined with F-MO-1's matrix gap,
  an Editor-can-manage-members regression passes everything), non-member blob
  upload/download, non-member snapshot GET/PUT (404 mapping), viewer blob upload.
- Fixed: a small role×route matrix loop hitting each guarded route as
  owner/editor/commenter/viewer/non-member and asserting the exact status.

### F-AP-3 — C, LOW — Authorization-header parsing tested only for the absent case
- `require_auth` (`src/auth_mw.rs:27-32`) rejects non-`Bearer ` schemes and
  non-UTF8 headers; only "no header" → 401 is tested (:101-110). `Basic ...`,
  `bearer x` (lowercase), and `Bearer` (no token → empty string, which StubVerifier
  — unlike production — rejects) are unexercised.

### F-AP-4 — C, LOW — Tier-2 attachment passthrough branch untested
- `tier01_attachments_are_sealed_at_rest_and_round_trip` (:439-503) covers the
  Tier-0 sealed branch well (asserts ciphertext-at-rest against the raw object
  store — a genuinely good instrument). The Tier-2 branch of
  `seal_for_rest`/`download` (`dek_wrapped: None` → stored/served as received,
  `routes/blobs.rs:53-59, 71-77`) is never exercised.

Healthy: 401 problem-type asserted, 404-not-403 for non-members (deliberate
information-hiding, asserted), tier-2 export 409 with the canonical problem type,
tier-0 export honestly 501, tier-2 member grant requires the DEK wrap (422 then
201 — guard false and true), snapshot forward-only incl. the stale-upload 409, GDPR
erase asserts identity severance (new user_id on re-login), validation 422.

---

## loki-server (0 tests)

### F-SV-1 — C, HIGH — sovereignty/config validation logic has zero tests
- File: `loki-server/src/config.rs:104-201`
- `ServerConfig::from_env` implements the ADR-C019 gates: Tier 2 rejected as
  deployment default (:130-135), residency EU pin (via `Residency::parse`),
  exactly-one-of `LOKI_OIDC_JWKS_URL`/`LOKI_OIDC_RSA_PEM_FILE` (:144-159), KEK
  base64 + 32-byte validation, compact-interval `0`-disables and `min_entries >= 1`
  floor. All of it is pure env→Result logic, eminently unit-testable, and none of
  it is tested — a regression accepting `LOKI_DEFAULT_TIER=2` (silently making
  zero-knowledge the default and breaking every server-side capability) or
  accepting both OIDC key sources would ship silently.
- Fixed: unit tests with a scoped env-var harness for each rejection arm and the
  defaults. (`main.rs` wiring is thin and reasonably left to integration.)

---

## Per-crate stats

| Crate | Test files read | Tests examined | A | B | C |
|---|---|---|---|---|---|
| loki-crypto | 4 | 13 | 0 | 0 | 3 |
| loki-server-audit | 2 | 7 | 0 | 0 | 3 |
| loki-server-auth | 3 | 11 | 0 | 0 | 4 |
| loki-model | 4 | 10 | 1 | 1 | 1 |
| loki-server-collab | 5 | 14 | 0 | 1 | 3 |
| loki-server-store | 2 | 2 | 0 | 1 | 3 |
| loki-server-api | 1 | 8 | 0 | 0 | 4 |
| loki-server | 0 | 0 | 0 | 0 | 1 |
| **Total** | **21** | **65** | **1** | **3** | **22** |

(Production files read to verify claims: all of loki-crypto, loki-server-audit,
loki-server-auth src; collab compact/relay/bus_pg/ws/hub/msg/bus_memory; store
ports/blob/memory/* and pg/* plus migrations 0001-0002; api auth_mw + all routes;
server config.rs.)

## Healthy areas

The suites here are far better than typical: crypto has real wrong-key, wrong-AAD,
tamper, truncation, and zero-knowledge (wrap-only-cannot-unwrap) negatives; the
audit chain has five distinct tamper scenarios with exact typed errors; JWKS has a
counted throttling test that would catch a DoS regression; the relay denial test
asserts the oplog stayed empty (not just the error); compaction tests use real Loro
payloads and assert merged content; and the API tests assert ciphertext-at-rest by
reading the raw object store underneath the handler — a textbook "instrument placed
where the hazard is". The dominant failure mode is not weak tests but **missing
inversions at the highest-stakes points** (signature validation, the RBAC matrix's
untested cells, the compaction race branch) and **whole layers with no coverage**
(PgStores, PgNotifyBus, drive_socket, server config).
