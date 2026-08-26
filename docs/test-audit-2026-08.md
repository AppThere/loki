# Test-efficacy audit — August 2026

Audited at commit `954433fd` (branch `claude/apply-verify-patch-2ls26x`), 2026-08-23 → 2026-08-25.
**Audit only — no fixes have been applied.** Static analysis; no builds or test runs.

**Scope:** every `#[test]`/`#[tokio::test]` in all 45 workspace crates — ~601 test files,
~3,670 test functions examined. Method: every test file was read in full; each suspicious
test was verified against the production code it calls, with the mutation
thought-experiment applied ("would this test fail if the production function were stubbed
or its guard inverted?"). Findings marked HIGH were verified by reading the production
source; MEDIUM are plausible but not fully traced.

**Categories:**

- **A — cannot fail:** the test passes regardless of production behaviour (tautologies,
  no assertions, self-referential expectations, silent skips).
- **B — doesn't test what it claims:** mislabeling or wrong methodology (reimplemented
  logic, symmetric round trips, presence-only assertions where the name promises fidelity).
- **C — not exhaustive:** material coverage gaps (untested error paths, one-sided guards,
  match arms with no test, whole untested layers).

---

## Executive summary

**234 findings: 35 A, 77 B, 122 C.** The workspace's overall test discipline is well
above typical — most suites show deliberate guard inversion, fixture preconditions, and
polarity pairs (several agents independently called their crates "the strongest suite
audited") — but the defects that do exist cluster at the **highest-stakes points**:

1. **The visual-fidelity axis largely cannot fail.** `loki-vello`'s only pixel test scans
   a directory that does not exist in the repo and returns green; all three DOCX
   visual-golden tests skip on `PENDING.txt`; `loki-acid`'s advertised page-count and
   glyph-coverage canaries pin no expected values; and no test anywhere exercises
   `Tolerance::calibrated()` against a known-bad render — loosening it to SSIM 0.0 keeps
   the whole workspace green.
2. **Security inversions are missing exactly where they matter most.** No test presents a
   JWT signed with the wrong key (disabling signature validation passes the suite); the
   RBAC matrix covers ~13 of 32 role×action cells (moving `Delete` into the Editor arm
   survives every test in the workspace); `loki-server` has zero tests over its ADR-C019
   sovereignty gates; the ADR-C013 compaction `LostRace` branch has no coverage.
3. **Persisted wire formats are unpinned.** Crypto blobs, the audit-log hash, the
   revision-mark encoding, and the CRDT mark vocabulary are all tested only by symmetric
   round trips through the same implementation — a coordinated format change passes every
   test while orphaning every persisted document, oplog, and audit chain.
4. **Format round-trip methodology has a systematic blind spot.** The `loki-ooxml`
   "divergence" round-trips compare cycle-1 vs cycle-2 (both post-export), so a property
   dropped entirely on first export passes; `loki-odf`'s only ODS round-trip masks a total,
   unmarked metadata loss in production.
5. **A handful of tests reimplement the production logic they claim to test** (texture
   budget mapping, window-geometry validation, an oversized-element guard, a platform
   probe) — the "one fact, one derivation" violation in test form.

The audit also surfaced **production defects** (§ Production defects below) — 10 as first
written, of which one (D-3) was retracted on inspection and one (D-10) downgraded, plus two
more found while remediating, leaving 11 standing. The sharpest are a parser stack-overflow
DoS in `loki-basic` that the fuzz suite deliberately avoids, an ODS export/import path that
silently drops all document metadata, and an error-28 recursion guard set so far above the
stack limit that it could never fire (D-12) — a guard that was not merely untested but had
never once executed.

### Totals by crate group

| Group | Files | Tests | A | B | C | Total |
|---|---|---|---|---|---|---|
| loki-text (5 partitions) | 90 | 626 | 7 | 18 | 24 | 49 |
| loki-doc-model | 90 | 522 | 2* | 6 | 4 | 12 |
| loki-ooxml | 77 | 381 | 2 | 6 | 5 | 13 |
| loki-layout | 46 | 389 | 4 | 9 | 3 | 16 |
| loki-odf | 41 | 297 | 1 | 5 | 4 | 10 |
| appthere-ui | 53 | 378 | 2 | 2 | 6 | 10 |
| Scripting (basic/macro-host/macro-sig/vba/fountain/markdown) | 56 | 386 | 0 | 8 | 15 | 23 |
| Export (epub/pdf/opc/convert/print/headless/graphics) | 37 | 147 | 2 | 8 | 14 | 24 |
| Server (server-*/model/crypto) | 21 | 65 | 1 | 3 | 22 | 26 |
| Rendering (canvas/conformance/renderer/vello/render-cpu/acid/bench) | 55 | 259 | 12 | 6 | 6 | 24 |
| App shell + small crates (app-shell/spell/spreadsheet/primitives/…) | 35 | 221 | 2 | 6 | 19 | 27 |
| **Total** | **601** | **3,671** | **35** | **77** | **122** | **234** |

\* one A finding in loki-doc-model is a cluster of ~30 tautological constructor tests.

---

## Cross-cutting patterns

These recur across crates and should be fixed as *patterns*, not one test at a time.

**P-1 · Silent skip = green.** Three separate implementations of "skip if goldens/fixture
absent" (`loki-vello/tests/visual_conformance.rs`, `loki-render-cpu/tests/visual_golden_docx.rs`,
`loki-acid/tests/golden_pixel.rs`), plus `loki-odf/tests/synthetic_style_leak.rs:189`
(returns Ok when the committed fixture is missing), `loki-layout/src/measure_tests.rs`
(all six tests return early on hosts without Liberation/DejaVu), and
`appthere-conformance`'s xmllint suite (4 of 6 tests self-skip when libxml2 is absent).
None are visible in the test summary. Per evidence rule 6, the marking must be
mechanical: `#[ignore = "…"]`, a hard failure under `CI=1`, or generating the input
in-test.

**P-2 · Symmetric round trips as the only guard for persisted formats.**
`loki-crypto` (blob layout, HKDF construction), `loki-server-audit` (`compute_hash`
domain-separator/field order), `loki-doc-model` (revision-mark encoding, CRDT mark
vocabulary, opaque-snapshot JSON), `loki-macro-host` (`trust/hex` nibble order,
capability-id list). Fix pattern: **one golden/known-answer assertion per persisted
format** (the crate's own `decode_tests.rs::v1_border_strings_still_decode` and
loki-vba's `abc_matches_the_decompressor_test_vector` show the working form).

**P-3 · Divergence-only round trips are blind to first-export loss.**
`loki-ooxml/tests/conformance_round_trip.rs` (3 tests) and
`conformance_xlsx_round_trip.rs` compare `import(export(seed))` against
`import(export(import(export(seed))))` — both post-export. Fix pattern: assert the named
property against the **seed** after one cycle (as `conformance_p0_round_trip.rs` already
does), keeping divergence as a backstop.

**P-4 · Presence-only assertions where the name promises a value.** ZIP-magic-only export
tests (`loki-ooxml/src/docx/export.rs`, `loki-convert` ODS↔XLSX), `contains('%')` for a
percentage, `is_some()` for a parsed border, `!header_items.is_empty()` for "page 1 gets
the first-page header", `byte_offset > 0` for "End moves to end of line". Fix pattern:
assert the discriminating value the fixture already carries.

**P-5 · The test reimplements the logic under test.**
`loki-text/src/texture_budget_tests.rs` (re-implements `current()`'s mapping byte-for-byte),
`loki-app-shell/src/window_geometry.rs:95` (re-declares the bounds closure),
`loki-text/dom_reflow/oversized_tests.rs` (restates the `fittable && expanded` guard),
`appthere-ui/src/components/platform.rs:72` (expected value computed by the production
expression). Fix pattern: extract the decision into a named production function and test
that (rule 4/5).

**P-6 · One-sided guards at security boundaries.** Wrong-key JWT (never tested),
algorithm confusion (never tested), RBAC deny cells (~19 of 32 untested), macro network
gate (`MACRO_NET_COMPILED && network_enabled` — deleting the check fails nothing), spell
consent-gate ordering (a gate moved after the download passes), `loki-convert` macro
*preservation* (only the drop-with-warning side is tested).

**P-7 · Whole layers with zero coverage.** `loki-server` (0 tests; sovereignty gates),
`loki-server-store/src/pg/*` (~600 lines of SQL, tested only via the in-memory double,
with one verified behavioural divergence), `PgNotifyBus`/`drive_socket`, the IPP wire
path in `loki-print`, `loki-layout`'s list-counter machinery and `flow_line_numbers::emit`,
`loki-renderer::paint_tile`'s reflow branch, `loki-text`'s `navigate_up/down` positive
paths and `caret_rect_*`.

**P-8 · Tests coupled to the developer's machine.** `loki-app-shell` spell-service tests
bootstrap against the real personal dictionary and dictionary store (assertions like
`!is_correct("teh")` flip on machine state); `recent_documents` loads the live profile;
`window_state`'s "no persisted file" test reads whatever `window.json` exists.

---

## Production defects surfaced by the audit

Found while verifying tests; each needs a root-cause fix, not just a test.

| # | Location | Defect |
|---|---|---|
| D-1 | `loki-basic/src/parser/` | No recursion-depth guard; deep `(`-nesting aborts the process. The panic-freedom fuzz test caps nesting at 200 explicitly to avoid it. |
| D-2 | `loki-odf/src/ods/{import,export*}.rs` | ODS drops `DocumentMeta` entirely (import hardcodes `default()`, export writes no `meta.xml`) — unmarked data loss, masked by the round-trip test. |
| ~~D-3~~ | ~~`loki-text/src/routes/editor/print_dialog_support.rs:26`~~ | **RETRACTED (2026-08-26) — not a defect.** The claim was that `"0"` forwards `copies: 0` to the IPP job. It does not: `PrintOptions::copies` is `u32`, documented "`0`/`1` are both one copy", and `ipp_attributes()` emits the attribute only when `copies > 1`, so zero never reaches the wire (IPP `copies` is integer(1:MAX)). The audit read the dialog's parse in isolation and never checked the encoder — instrument failure of the fourth kind: correctly placed, but reporting on a quantity adjacent to the one asked. The *test* gap was real and is now closed on both sides (`zero_and_one_copies_emit_no_copies_attribute` in loki-print, `a_zero_copies_field_never_becomes_a_zero_copy_job` in loki-text). |
| D-4 | `loki-opc/src/part/name.rs:97` | `extension()` rsplits the whole path, so `/v1.0/data` → `Some("0/data")`; feeds `ContentTypeMap::resolve`. |
| D-5 | `loki-primitives/src/color/document.rs:77` | `from_hex` accepts garbage alpha digits (`"#123456ZZ"` parses; last two bytes never validated). |
| D-6 | `loki-server-api/src/routes/documents.rs:22` | `create` checks only workspace existence — any authenticated user can create docs in any workspace (known `TODO(ws-membership)`, but unpinned by any test either way). |
| D-7 | `loki-server-store` | Memory double diverges from Postgres on `upsert_user_by_oidc` display-name refresh (memory discards, Pg updates); also `pg/user.rs:48` `unwrap_or(false)` silently drops the once-per-account AuthLogin audit on decode failure. |
| D-8 | `loki-server-audit` | `verify_chain` has no production caller — tamper evidence is never checked by the running system (placement, rule 7). |
| D-9 | `loki-text/src/editing/touch.rs` | `TouchPhase::Tap` is matched but constructed nowhere — dead state with a test named for it (parked-vs-forgotten, rule 6). |
| D-10 | `loki-server-auth/src/verifier.rs:52` | **DOWNGRADED (2026-08-26) — intended, not a vulnerability; now pinned.** `StaticKeys` does serve the default key for any unknown kid, but the only production caller is `StaticKeys::single(...)`, which builds an *empty* kid map plus a default key: rejecting unrecognised kids would break static-PEM mode against every IdP that stamps a `kid`. Nor is it a bypass — the signature is still verified against the operator-installed key, so a forged kid cannot launder a foreign signature (now asserted). The genuine defect alongside it was a doc one: `KeySource::key_for`'s trait doc claimed a `None`-kid-only fallback the impl does not honour (rule 4), since corrected. |

### Found during P0 remediation (2026-08-26)

| # | Location | Defect |
|---|---|---|
| D-12 | `loki-basic/src/interp/mod.rs:45` (`MAX_CALL_DEPTH`, enforced at `interp/call.rs:188`) | **The error-28 guard was set above the hazard it guards, so it was unreachable.** Writing LB-3's missing test showed the guard is worse than untested: one interpreted call costs ~19 KiB of native stack, so at `MAX_CALL_DEPTH = 256` the process aborted on stack overflow at roughly depth 110 — about 2.5× before "Out of stack space" could ever fire. Unbounded macro recursion therefore killed the host rather than raising a trappable VBA error, which is precisely what the guard exists to prevent. Fixed by lowering the limit to a measured 32. This is the sharpest illustration in the audit of why an untested guard is not merely unverified: it had never once executed. *Behaviour change:* legitimate recursion deeper than 32 now raises error 28 (trappable) earlier than before; running macros on a dedicated large-stack thread would let the limit rise, and is the recommended follow-up. |
| D-11 | `loki-doc-model/src/loro_bridge/inlines.rs` (`map_inlines`) | **Character marks bleed across the rest of the paragraph on initial serialization.** `Block::Para([Strong(["bold"]), Str(" normal")])` serialises to a single span `Insert { insert: "bold normal", attributes: {"bold": true} }`, and reads back as one bold run. Cause: `map_inlines` inserts each inline's text and applies its marks immediately, then inserts the next inline at the trailing edge — and every char mark is registered `ExpandType::After` (`loro_bridge/compact.rs:42-49`), so the mark swallows what follows. It cascades: bold, then bold+colour, then bold+colour+revision leak forward through the whole paragraph. `loro_mutation::text::replace_text` documents and defends against exactly this hazard; the writer has no equivalent defence. **Effect:** any imported document with a formatted word followed by plain text in the same paragraph loses its run boundary the moment it enters the CRDT. **Not established:** whether this is known or deliberate (no TODO/ADR/docs entry found), the correct fix, and whether any export path masks it. **What would settle it:** a test asserting `Block::Para([Strong(["bold"]), Str(" normal")])` yields two runs; the likely fix is a two-pass `map_inlines` (insert all paragraph text, then apply marks over computed ranges) or clearing the expanded keys per span as `replace_text` does. Not fixed — out of scope for P0 and a wide blast radius. |

Two of this report's own claims were also disproved while acting on them, beyond the D-3
retraction and D-10 downgrade above:

- **loki-doc-model F7** stated the `\u{1f}` separator was "indirectly pinned by
  `an_unknown_tag_is_rejected`". It is not — that test survives a separator change. The
  separator was entirely unpinned until the golden added in this pass.
- **scripting-formats MH-3** stated capability ids "key persisted trust-store grants".
  They do not: `PersistedGrant` serialises the *variant* name (`"ClipboardWrite"`), not the
  id (`"clipboard-write"`); ids are used for i18n key suffixes and author-visible error
  text. Both the id list and the serde variant names are now pinned, and `id()`'s
  doc comment — which claimed it was "used as the serialized key" — was corrected.

---

## Remediation status

**P0 was worked 2026-08-25/26.** Every item below was verified per-crate (`cargo test`,
the CI clippy flags, `cargo fmt`, the ceiling and licence gates), and — the part that
matters for an audit about tests that cannot fail — **each new guard was mutation-checked
against the specific mutation this report claimed would survive**. The mutations that now
die include: moving `Action::Delete` into the Editor arm; disabling JWT signature
validation; removing the audit hash's length prefixes; swapping the HKDF salt halves;
a constant `seal` nonce; renaming `MARK_BOLD`; changing the revision-mark separator;
skipping the compaction loser's blob delete; and truncating the oplog before the
forward-only guard. Where a mutation did *not* die, that is recorded above as a
retraction (D-3) or downgrade (D-10) rather than quietly dropped.

| P0 item | Status |
|---|---|
| 1 · Wrong-key + algorithm-confusion JWT tests | Done — `loki-server-auth` 11 → 17 tests; disabling signature validation now fails 4 tests and 0 pre-existing ones. D-10 pinned, not changed (see above). |
| 2 · Exhaustive RBAC matrix | Done — all 32 `Role`×`Action` cells against a hand-written ADR-C017 table, with a compile-time gate for new variants; plus a role×route denial matrix (`loki-server-api`, 8 → 19 tests). The cross-workspace create probe returns **201 CREATED**, now pinned with a `TODO(ws-membership)` comment so the deferral fails loudly when fixed. |
| 3 · Known-answer vectors for persisted formats | Done — `loki-crypto` (blob layouts, HKDF derivation, JSON encoding, nonce freshness), `loki-server-audit` (hash KAT, µs precision, length-prefix forgery), `loki-doc-model` (revision-mark golden + a committed 4,657-byte Loro snapshot fixture pinning the mark vocabulary), `loki-macro-host` (capability ids, serde variant names, hex codec). All vectors are captured from today's implementation and commented as such — they freeze the format, they do not independently validate the primitives. |
| 4 · `loki-server` config tests | Done — 0 → 27 tests over every ADR-C019 rejection arm, via a pure `from_vars` seam. (The refactor also fixed a latent panic: `std::env::vars()` aborts on any non-UTF-8 variable.) |
| 5 · Compaction `LostRace` branch | Done — the branch is now actually reached via an interposing store wrapper; all three properties (outcome, loser-blob deleted, oplog untruncated) hold and each is independently load-bearing. Plus 11 `PgNotifyBus` envelope tests. No ADR-C013 bug found. |
| 6 · Bomb/recursion guards | Done — D-1 confirmed live (the reproducer aborted with `fatal runtime error: stack overflow`, taking the whole test binary with it, before any fix) and closed with a stack **budget** rather than a level count, because measurement showed per-level cost varies ~4× by construct and this report's suggested limit of ~256 was unsafe by 3×. Two parser recursion cycles the audit did not name were also found and charged. Uncovered D-12 (above). `loki-vba` went from 5 tests to 16, with each bomb guard tested on both sides of its boundary and every guard mutation-killed by exactly its intended test. |
| 7 · Production fixes D-2…D-5 | Done — D-2 (ODS metadata) and D-4 (`PartName::extension`) and D-5 (`from_hex` alpha) fixed test-first, each reproducer failing before the fix. D-3 retracted as a false finding. |

Two environmental notes from the pass: `xmllint` was absent on the audit machine, so five
`loki-odf` schema-validation tests were failing for reasons unrelated to any change
(installing `libxml2-utils` turned them green — and `odt_meta_xml_is_schema_valid` then
validated the shared `meta.xml` writer against the real ODF 1.3 RELAX-NG schema); and the
D-2 fix initially pushed `ods/import.rs` to 301 lines, caught by the file-ceiling gate.

## Priority-ranked remediation

### P0 — security and persisted-data integrity

1. **Wrong-key + algorithm-confusion JWT tests** (`loki-server-auth`): a token signed
   with a different secret against the standard verifier must fail; HS384 against an
   HS256-only verifier must fail. Also pin the `StaticKeys` unknown-kid fallback (D-10)
   and "old kid stops verifying after rotation".
2. **Exhaustive RBAC matrix** (`loki-model/src/role.rs`): a 4×8 loop against a literal
   expected table; plus an API-level role×route denial matrix (`loki-server-api`) and a
   probe pinning D-6's intended interim behaviour.
3. **Known-answer vectors for persisted formats**: crypto blob layout + wrapped-DEK JSON
   (`loki-crypto`), audit hash + length-prefix forgery defense (`loki-server-audit`),
   revision-mark encoding + a checked-in binary Loro snapshot golden (`loki-doc-model`),
   `trust/hex` + capability-id list (`loki-macro-host`). Add a nonce-uniqueness check
   (two seals differ).
4. **`loki-server` config tests** (0 → tests for every ADR-C019 rejection arm: Tier-2
   default rejected, residency pin, exactly-one OIDC key source, KEK validation).
5. **Compaction `LostRace` branch** (`loki-server-collab`): interleaved
   `set_snapshot` → assert `LostRace`, loser blob deleted, oplog NOT truncated. Plus
   `Notification` serde / oversized-awareness units for `PgNotifyBus`.
6. **Bomb/recursion guards must be killable**: `loki-basic` depth guard (D-1) + test at
   depth 5000 asserting `Err`; call-depth error 28 test; `loki-vba` chunk/output-cap
   fixtures that actually reach the guards.
7. **Production fixes with their tests**: D-2 (ODS meta), D-3 (copies floor), D-4
   (extension()), D-5 (from_hex alpha).

### P1 — fidelity gates that currently cannot fail

8. **Make golden skips loud** (P-1 list): `#[ignore]` or hard-fail-in-CI for the vello
   conformance scan, DOCX goldens, acid renders, measure_tests host fonts, xmllint, and
   the odf committed-fixture test.
9. **Give the calibrated tolerance a floor** (`appthere-conformance`): one known-bad pair
   must fail at `Tolerance::calibrated()`; assert the constants against the calibration
   record.
10. **Pin the acid canaries**: independently-verified expected page counts per fixture;
    bundled-font zero-tofu assertion (host-independent) in place of the always-true
    `notdef <= total`.
11. **Fix divergence-only round trips** (`loki-ooxml` B-1/B-2, P-3 pattern) and replace
    ZIP-magic-only assertions (A-1, CV-1) with content assertions; assert the
    first-page-header *variant* by text (B-3).
12. **loki-layout's untested machinery**: list counters (discriminate via glyph counts or
    `preserve_for_editing`), keep-together/keep-with-next fixtures where the keep flag
    must move a block that *fits* (F2/F3/F15), `flow_line_numbers::emit`, fragment-B clip
    `y ≈ 0`.
13. **loki-doc-model**: six-slot header/footer round trip with distinct sentinels (F5);
    destructuring-based `merged_with_parent` exhaustiveness (F6); adversarial
    corrupt-CRDT reader suite (F8).

### P2 — hygiene and completeness

14. Delete or repair the tautology clusters (doc-model constructor tests, wgpu_surface
    "integration" file — rename it, layout `!w.is_empty() || w.is_empty()`, discarded
    `matches!` in loki-graphics, `let _ = get_block_style_name(...)`).
15. De-machine-couple `loki-app-shell` tests (injectable paths/`bootstrap_with_personal_dict(None)`).
16. Fill the enumerated C-gaps in the per-crate files: one-sided clear/impact arms in the
    style inspectors, `loki-basic` statement families (`ReDim Preserve`, `Exit For`,
    `Resume`…), markdown/fountain emission assertions, EPUB heading-style mapping and
    inline wrappers, IPP wire tests, spreadsheet lone-ref-as-range + `#VALUE!` arms,
    `Affine2::rotation(π/2)` + `then` composition order, i18n primary-locale-wins.

---

## Per-crate detail

The full findings — with file:line, test names, quoted evidence, and the assertion a
fixed test would make — are in the per-crate audit files under
[`docs/test-audit-2026-08/`](test-audit-2026-08/); this section condenses each to its
worst items.

### loki-text (A 7 · B 18 · C 24)

Worst: `tests/wgpu_surface_integration.rs` — three tautologies and a filename claiming
wgpu coverage that doesn't exist; `texture_budget_tests.rs` re-implements `current()`'s
mapping (P-5); Home/End navigation asserts satisfied by an identity stub
(`byte_offset <= 6` / `> 0`); `navigate_up/down` positive paths (~80 lines incl. the
Bug-3 guard) untested; macro-apply suite can't distinguish deleted from emptied blocks;
"one undo entry" test never asserts undo granularity; `a_gone_ui_degrades_to_deny`
answers its own prompt so the real closed-channel branches are unreached;
`style_impact`/`clear_local_property` cover 2/11 and 4/11 arms; print copies `"0"` (D-3);
touch `word_at_boundary` accepts both outcomes of the code under test. Healthy: the
dialog suites (page/span/table), spell placement, wheel-zoom, hit-test, and
page-locate/characterisation suites are exemplary.

### loki-doc-model (A 2 · B 6 · C 4)

Worst: only the Default *header* of six header/footer slots is round-trip tested (F5);
`merged_with_parent`'s 30/26-line hand lists spot-checked at 3 and 1 fields (F6); ~90
`loro_to_document` call sites all expect `Ok` — reader failure behaviour on malformed
remote CRDT state entirely unverified (F8); revision-mark and mark-vocabulary wire
formats unpinned (F7/F13); ~30-test tautological constructor cluster (F9). Healthy:
asymmetric CRDT-side probes, selection/mutation error paths, concurrency suites.

### loki-ooxml (A 2 · B 6 · C 5)

Worst: three divergence-only round trips blind to first-export loss (B-1; the
emboss/imprint/`w:bdr` claim has no fixture anchor at all); four export tests assert only
`PK` and are the *only* coverage of the `BulletList`/`OrderedList` export arms (A-1 +
C-2); header-variant test never checks which variant a page got (B-3); XLSX import has
zero error-path tests (C-1). Healthy: table-style/page-style round trips, schema
validation with a deliberate-failure control, repair suite, OMML goldens.

### loki-layout (A 4 · B 9 · C 3)

Worst: the entire list-counter machinery is unobserved (F5/F6/F7 — stub `advance_counter`
to a constant and nothing fails); keep-together/keep-with-next fixtures overflow
naturally so the keep logic can be deleted (F2/F3/F15); `flow_line_numbers::emit`
untested (F14); a literal `!w.is_empty() || w.is_empty()` tautology; fragment-B clip
asserts `y >= 0` under a name claiming "top of next page"; measure_tests silently skip
without host fonts (F10). Healthy: column flow, spacing collapse, borders, caches,
font substitution, header variants, boundary coverage.

### loki-odf (A 1 · B 5 · C 4)

Worst: the only ODS round-trip masks total metadata loss (F1/D-2) and canonises a
formula-spelling asymmetry (F2); committed-fixture leak test passes vacuously when the
fixture is absent (F3); tracked changes/comments/math/columns/headers verified only
symmetrically while the schema gate validates a minimal document (F4); `styles.xml`-absent
fallback has zero coverage (F5). Healthy: ODT error paths, anti-vacuity controls,
discriminating border/style values. The ODS half has a fraction of the ODT half's rigor.

### appthere-ui (A 2 · B 2 · C 6)

Worst: popover mount-order tests assert `RootLayer::mount_order()`, a table no production
code reads — swapping the divs (the exact "dead menu" failure described) passes (B-1);
`the_host_render_never_calls_place` goes vacuous on rename (B-2); two tautologies
(platform detect, macOS budget arithmetic); `status_bar_overflow` pure logic and the
macro-security prompt defaults untested. Otherwise the strongest suite in the workspace
(systematic L08-045 inversion partners, mutation-derived cases, in-test fixture
preconditions).

### Scripting crates (A 0 · B 8 · C 15)

Worst: `loki-basic` parser depth hazard (D-1/LB-4) and untested call-depth guard (LB-3);
whole statement families with zero execution tests (LB-5); `Find.Execute`'s boolean is
unobservable through `RunOutcome` so the find suite can't see the report (MH-1); MsgBox
test asserts neither gating nor rendering (MH-2); capability-id stability unpinned though
ids key persisted grants (MH-3); `loki-vba` bomb-guard test has no assertion and can't
reach the guard (MV-1); raw-chunk branch never executed (MV-3); fountain page-break/
centered emission and markdown table/nesting content unasserted. Healthy: the security
posture suites (refusals, broker, trust store, CMS/XMLDSig) are the strongest in the
workspace; no category-A findings at all.

### Export crates (A 2 · B 8 · C 14)

Worst: ODS↔XLSX "round trip" asserts only ZIP magic on an empty fixture (CV-1); PDF/X
flattening asserted only by mask absence — stubbing the over-white compositing passes
(P-1[pdf]); the level→flatten wiring seam has no test so inverting the `!` is caught by
nothing (P-2[pdf]); loki-opc's only core-properties value assertion is behind a
non-default feature gate that doesn't even need it, with leftover debug prints (O-1);
reader error paths zero-tested (O-2); `extension()` bug (O-3/D-4); the entire IPP wire
path untested (PR-1); macro-preservation guard never false (CV-2). Healthy:
page-geometry conformance, PDF structural re-parse, EPUB container re-open suites.

### Server crates (A 1 · B 3 · C 22)

Worst: no wrong-signature or algorithm-confusion JWT test (F-AA-1/2); RBAC matrix ~13/32
cells, Editor+Delete mutation survives the workspace (F-MO-1); `lost_race…` test exits at
`NothingToDo` — the actual LostRace branch has zero coverage (F-CO-1); PgStores (~600
lines SQL), PgNotifyBus, drive_socket, and `loki-server` config all untested
(F-ST-1/F-CO-2/3/F-SV-1); no known-answer vectors for crypto or audit-hash formats
(F-CR-1/F-AU-1/2); `verify_chain` never called in production (D-8); create-document authz
gap unpinned (F-AP-1/D-6). Healthy: crypto negatives (wrong key/AAD/tamper/zero-knowledge),
audit tamper scenarios, JWKS throttling, ciphertext-at-rest instrument.

### Rendering crates (A 12 · B 6 · C 6)

Worst: `loki-vello`'s only pixel test permanently vacuous — fixture dir doesn't exist
(LV-1); six paint tests with zero assertions (LV-2); no test fails at
`Tolerance::calibrated()` for a known-bad pair (CF-1); acid page-count/glyph canaries pin
nothing and the real no-tofu gate is permanently `#[ignore]`d (LA-2/3); DOCX goldens all
`PENDING.txt` (LRC-1); `GpuTexture` drop-accounting and dhat measurement asserted only in
bench targets `cargo test` never runs (AC-2/LB-1); timing-controls ordering (the module's
reason for existing) untested (LB-2); budget-ceiling tautology (AC-1); severity-totals
partition tautology (CF-3). Healthy: the residency stack, hatch/data-URI tests, ODT
golden pipeline (LibreOffice-generated, provenance committed), bench verdict logic.

### App shell + small crates (A 2 · B 6 · C 19)

Worst: `implausible_dimensions_are_rejected` reimplements the validation and never calls
`load` (AS-1); five spell-service tests run against the developer's real profile (AS-2);
no save→load round trip anywhere in the crate (AS-4); spreadsheet lone-ref-as-range
branch deletable with no failure (SS-1); UDF display trimming duplicated in the harness
(SS-2); `Affine2::rotation` tested only at angle 0 and `then` order unpinned (PR-1/2);
`from_hex` alpha laxity (PR-3/D-5); i18n primary-locale-wins untestable with only en-US
embedded (I18-1); fonts "same set" check is a count (FT-1); presentation
create-missing-body arm dead in tests (PP-1). Healthy: display density/calibration,
measurement, tokenizer, cell_ref, templates.

---

## What would settle it (audit-level)

- **Observed:** the findings above, each verified against production source at the stated
  file:line (HIGH) or flagged MEDIUM.
- **Not established:** actual mutation-kill rates (no `cargo mutants` run was performed —
  this was static analysis); whether the D-* production defects reproduce at runtime
  (each was traced in source but not executed).
- **What would settle it:** run `cargo mutants` (or targeted hand mutations) on the P0/P1
  areas to confirm the survive-claims; execute reproducers for D-1…D-5; then land fixes
  test-first so each finding's "fixed test would assert" line turns red before the fix
  and green after.
