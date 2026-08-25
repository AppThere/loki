# Test-efficacy audit — rendering / canvas / fidelity-harness crates

Scope: appthere-canvas, appthere-conformance, loki-renderer, loki-vello,
loki-render-cpu, loki-acid, loki-bench. Static analysis only (no builds/runs).
Categories: **A** cannot fail, **B** doesn't test what it claims, **C** material gap.
Confidence: HIGH = production code read and traced; MEDIUM = plausible, not fully traced.

---

## appthere-canvas

### AC-1 — `no_diagnostic_ceiling_leaves_every_arm_alone` is a tautology
- **File:** `appthere-canvas/src/residency/budget_tests.rs:374-398`
- **Category:** A · **Confidence:** HIGH
- **Defect:** The test compares `derive(X)` against `derive(X)` — both arguments are the identical struct value — so it passes for any implementation of `derive`.
- **Evidence:** Every fixture is built with `..Default::default()`, so `diagnostic_ceiling_bytes` is already `None`; the "control" then rebuilds the same value:
  ```rust
  let with_none = TextureBudget::derive(BudgetInputs { diagnostic_ceiling_bytes: None, ..inputs });
  assert_eq!(TextureBudget::derive(inputs), with_none);
  ```
  `BudgetInputs` is `Copy + PartialEq`; `inputs == with_none` holds before `derive` is ever called. The doc comment claims "Without this the lever could be a constant and both assertions above would still pass" — but this test also passes in that mutation (its two calls stay equal to each other).
- **Fix:** Assert the *value* of the unforced ceiling against the device-derived expectation, e.g. `derive(inputs).hard_ceiling_bytes() == survival expectation for that RAM figure`, or compare against a run that *did* set a diagnostic ceiling and assert inequality of the ceilings (the neighbouring test already covers half of this; this one should be deleted or rewritten to pin absolute ceilings per arm).

### AC-2 — `GpuTexture` structural residency accounting is never exercised by any test
- **File:** `appthere-canvas/src/texture.rs:47-79` (no `#[cfg(test)]` anywhere in the file)
- **Category:** C · **Confidence:** HIGH
- **Defect:** The crate's headline claim — "Construction records the allocation … `Drop` records the release, so a texture obtained this way cannot escape the count" — has no test; the counter is tested only with hand-fed `record_alloc/record_free` calls (`counter_tests.rs`), i.e. the test's own accounting, not the type's.
- **Evidence:** `GpuTexture::new` / `Drop` call `TextureResidency::record_*`, but constructing one needs a `wgpu::Texture`; the only checks of the pairing live in `loki-bench/benches/texture_residency.rs`, which `cargo test` never runs. Deleting the `Drop` impl would fail no test in the workspace's test phase.
- **Fix:** The byte arithmetic (`texture_bytes(width,height)` recorded on construction, identical figure freed on drop) does not need a real GPU to pin — a test-only constructor or a feature-gated fake `wgpu::Texture` is impossible, so at minimum add a unit test on `loki-renderer`'s hand-recorded alloc/free sites (see LR-6) or promote the bench's balance assertion into an ignored-by-default test that CI's GPU runner executes.

### AC-3 — stale wiring claim under `visible_window` (note, not a test defect)
- **File:** `appthere-canvas/src/residency/geometry.rs:241-243`
- **Category:** B (doc-level) · **Confidence:** HIGH
- **Defect:** The doc comment sells the tests as covering production because "`loki_renderer::virtualize` calls it — not a restatement of it"; no function `virtualize` exists in `loki-renderer`, and nothing outside this crate calls `visible_window` directly.
- **Evidence:** Grep over `loki-renderer/src` + `loki-text/src` finds no `visible_window` caller; production reaches it only transitively via `plan_residency` (`loki-renderer/src/tile_plan.rs:153`). The one-implementation property still holds (so the geometry tests do test production), but the stated call edge is stale and a future refactor of `plan_residency` away from `visible_window` would silently orphan `visible_window`'s tests.
- **Fix:** Correct the comment to name `plan_residency`/`tile_plan::plan_tiles` as the consumers.

Otherwise this crate's suite is exceptional: budget arithmetic pinned against independent literals, both polarities asserted (e.g. `the_overridden_ceiling_still_tracks_the_device` exists precisely to stop the previous test being satisfied by a constant), reachability/depth/ceiling sweeps drive the real planner, and fixtures carry preconditions ("this budget must actually put the plan under pressure").

---

## loki-renderer

### LR-1 — `the_cell_round_trips_and_starts_unobserved` never checks the cell's start state
- **File:** `loki-renderer/src/gpu_probe.rs:111-128`
- **Category:** B · **Confidence:** HIGH
- **Defect:** The name claims the cell "starts unobserved", but the body asserts only `decode(0) == None` — a fact about the decoder, not about the static's initial value.
- **Evidence:**
  ```rust
  assert_eq!(decode(0), None);           // decoder property
  for kind in [...] { ... record(kind); ... }
  ```
  Mutation: change `static OBSERVED: AtomicU8 = AtomicU8::new(UNOBSERVED)` to `AtomicU8::new(1)` — every assertion still passes, yet `observed_adapter()` would report `Some(Discrete)` before any paint (exactly the "looked and found nothing" confusion L9-009 forbids).
- **Fix:** Assert `observed_adapter() == None` as the first statement (safe: this is the only test in the binary touching the cell), *then* record.

### LR-2 — ambient measure-cap race between test files
- **Files:** `loki-renderer/src/measure_tests.rs:15-25` vs `loki-renderer/src/render_layout_tests.rs:145-215`
- **Category:** B (flake hazard / unstated precondition) · **Confidence:** HIGH (mechanism verified; flake frequency untested)
- **Defect:** `measure_tests` temporarily install process-wide content caps (`Some(300.0)`, `Some(10_000.0)`) under a mutex private to that file, while `render_layout_tests` (`narrow_viewport_uses_its_full_width`, `wide_viewport_caps_the_measure`, `content_width_is_tile_minus_insets_and_floored`, `type_scale_keeps_the_on_screen_tile_width_unchanged`, `compact_layout_measure_narrows_by_the_scale`) read the same ambient state via `reflow_tile_width_px → measure::max_tile_width_px()` with no lock. Both mods are in the same test binary and run on parallel threads.
- **Evidence:** `reflow_metrics.rs:41` — `viewport_width_px.clamp(0.0, crate::measure::max_tile_width_px())`. If a `measure_tests` cap of 300 pt (~448 px tile) is live when `wide_viewport_caps_the_measure` runs, `reflow_tile_width_px(2560.0)` returns ≈448, not `MAX_REFLOW_TILE_PX` — spurious failure; conversely these tests silently depend on "no cap installed", a precondition nothing asserts.
- **Fix:** Share one lock (e.g. expose `measure_tests::with_cap` as a crate-test helper, or take the same `static LOCK` from both files), or have the render_layout tests pin the cap to `None` through the same guard.

### LR-3 — `RenderLayout::paint_tile` reflow branch has zero coverage
- **File:** `loki-renderer/src/render_layout.rs:183-268`
- **Category:** C · **Confidence:** HIGH
- **Defect:** The reflow paint path — selection rects filtered to the band, the `height: 0.0` no-caret sentinel, band-local offset `(REFLOW_PADDING_PT, -band_top)`, caret-in-band intersection — is untested in this crate, and loki-vello's tests do not reconstruct it.
- **Evidence:** `render_layout_tests.rs` tests tiling arithmetic, hit-testing, and cursor canvas rects, but never calls `paint_tile`; no other test in the workspace calls it either (grep). Inverting the band filter (`r.origin.y <= band_top + band_h` → `<`, or dropping `-band_top`) would fail nothing.
- **Fix:** Call `paint_tile` on a two-band reflow fixture with a selection straddling the band boundary and assert the scene encoding is non-empty for both bands and empty for a band the selection does not reach (vello `Scene` exposes its encoding; even a coarse "painted vs not painted" discrimination would catch an inverted filter).

### LR-4 — expected values in measure/reflow tests duplicate the production formula
- **Files:** `loki-renderer/src/measure_tests.rs:45` (`(content_pt + 2.0*REFLOW_PADDING_PT)/PX_TO_PT`), `render_layout_tests.rs:168, 210, 236`
- **Category:** A (mirror-computation) · **Confidence:** HIGH · **Severity:** low
- **Defect:** The expected value is the production expression re-typed with the production constants, so a wrong *constant* (e.g. `REFLOW_PADDING_PT` mis-set) or a units error baked into both copies passes.
- **Evidence:** `measure.rs:86` computes `(content_pt + 2.0 * REFLOW_PADDING_PT) / PX_TO_PT`; the test computes the identical expression. Mitigation already present: the `< MAX_REFLOW_TILE_PX` polarity assert, and `us_letter_matches_the_s0_2_arithmetic`-style independent literals elsewhere.
- **Fix:** Pin one case as a literal (e.g. content 300 pt → tile 448 px at the current constants) so a constant change must be acknowledged in the test.

### LR-5 — reflow arm of `DocPageSource` untested
- **Files:** `loki-renderer/src/doc_page_source_reflow.rs` (no tests); `doc_page_source_tests.rs` covers only the paginated reuse arm
- **Category:** C · **Confidence:** MEDIUM
- **Defect:** Layout compute/caching for `RenderMode::Reflow` (width-keyed invalidation, generation interaction) has no direct tests; only `RenderMode::matches` width tolerance is tested.
- **Fix:** A test that switches a source Paginated → Reflow → width+0.4px (no relayout) → width+10px (relayout) and observes generation/cache identity, mirroring the paginated tests' `Arc::ptr_eq` technique.

Healthy: tile_key invalidation drives real texture arithmetic; scale_resolve tests assert layout *identity* (`Arc::ptr_eq`) with the documented regression and its polarity; doc_page_source_scale tests cover cap/uncap/restore and count memo recomputes per input; zoom_capability tests state their own limitation honestly and pin the always-`None` polarity (L08-045).

---

## loki-vello

### LV-1 — the crate's only integration test is permanently vacuous
- **File:** `loki-vello/tests/visual_conformance.rs:271-303`
- **Category:** A · **Confidence:** HIGH
- **Defect:** `test_visual_conformance` scans `tests/conformance/documents/`, which **does not exist in the repository**; the test creates the empty directories, prints "No test files found", and returns green. The GPU paint crate's only pixel-comparison test compares nothing, on every checkout, and has no marker (no PENDING file, no ignore attribute) distinguishing it from a real pass.
- **Evidence:** `ls loki-vello/tests/conformance/` → "No such file or directory". Test body: `if files.is_empty() { println!("No test files found…"); return; }`.
- **Fix:** Either commit fixtures + references (the loki-render-cpu ODT golden pipeline shows the working pattern), or make emptiness loud: `#[ignore = "no conformance fixtures committed"]`, or assert `!files.is_empty()` so the vacuity is a failure, not a green run. Note the file also duplicates a cruder pixel differ (10/255 channel threshold, 0.5% mismatch budget) beside the calibrated SSIM/ΔE machinery in appthere-conformance — if fixtures ever land, use `compare_pages` instead of this second, uncalibrated metric.

### LV-2 — paint tests assert nothing about the scene
- **File:** `loki-vello/src/scene_tests.rs:95-119, 159-169` (`test_paint_filled_rect`, `test_paint_with_scale_factor`); same pattern in `test_paint_empty_layout_does_not_panic`, `test_paint_clipped_group_does_not_panic`, `test_paint_rotated_group_does_not_panic`, `paint_decoration_every_style_does_not_panic`
- **Category:** A · **Confidence:** HIGH
- **Defect:** `test_paint_filled_rect` and `test_paint_with_scale_factor` claim (by name) to test painting but contain zero assertions — stubbing `paint_layout` to an empty body passes them. The `*_does_not_panic` tests are honestly named but leave the module with **no** test that painting produces any output at all.
- **Evidence:** each body ends `paint_layout(&mut scene, ...); // No panic = pass.` The `vello::Scene` is dropped uninspected.
- **Fix:** Assert scene content coarsely — e.g. `scene.encoding()` path/draw-object counts are non-zero after painting a filled rect, zero for an empty layout, and larger for two rects than one. That single discrimination would catch a dispatch arm silently dropping an item (exactly the defect class loki-render-cpu's `hatch_render.rs` documents as having shipped).
- Contrast: `translate_item` tests and `scene_cursor_tests` (transform pinned against `CellRotation::local_to_page`) are real; `band.rs`/`color.rs` tests are small but sound.

---

## loki-render-cpu

### LRC-1 — all three DOCX visual-golden tests are no-ops (documented)
- **File:** `loki-render-cpu/tests/visual_golden_docx.rs:104-117` (`acid_docx_matches_its_golden`, `iris_blueprint_matches_its_golden`, `acid2_docx_matches_its_golden`)
- **Category:** A (documented deferral) · **Confidence:** HIGH
- **Defect:** Every fixture's golden dir contains only `PENDING.txt` (verified: `appthere-conformance/goldens/docx/*/`), so `golden_pages` returns empty and `compare_fixture` returns after an `eprintln!` — three tests named "matches_its_golden" that have never compared a pixel.
- **Evidence:** `if goldens.is_empty() { eprintln!("…skipping"); return; }`. The module header documents this honestly, and the marking is semi-mechanical (PENDING.txt), so per the project's rule 6 this is a parked deferral — but the *test names* assert a property that is not being checked, and nothing outside stderr distinguishes these greens from real passes.
- **Fix:** Until PNGs land, gate with `#[ignore = "goldens pending (PENDING.txt)"]` so the skip is visible in the test summary rather than only in captured stderr.

Healthy: `render_smoke.rs` is a model smoke suite (real ink counted, byte determinism, error path); `hatch_render.rs` is exemplary — ink where the hatch is, paper between the lines, and the overshoot inversion; `image_tests.rs` includes the scheme-vs-payload control ("the control payload must decode, or the test above proves nothing"). The ODT goldens (`visual_golden.rs`) are genuinely independent (LibreOffice → pdftoppm, provenance in GENERATION.txt/CALIBRATION.md) — this is what the DOCX axis should converge to.

---

## appthere-conformance

### CF-1 — nothing fails if the calibrated tolerance is loosened to vacuity
- **Files:** `appthere-conformance/src/golden/calibration.rs:15-19` (`CALIBRATED_MIN_SSIM = 0.60`, `CALIBRATED_MAX_DELTA_E = 10.0`); consumers `loki-render-cpu/tests/visual_golden.rs`, `visual_golden_docx.rs`
- **Category:** C (missing inversion / gate floor) · **Confidence:** HIGH
- **Defect:** No test anywhere exercises `Tolerance::calibrated()` against a *known-bad* pair. `diff_tests.rs` uses its own stricter `tol()` (0.9 / 4.0); the golden tests only assert *pass*. Mutation: set `CALIBRATED_MIN_SSIM = 0.0` and `CALIBRATED_MAX_DELTA_E = 1000.0` — the entire workspace test suite stays green while the visual axis loses all discriminating power. This is exactly the project's own maxim that "a counting gate needs a floor as well as a ceiling."
- **Evidence:** grep: `Tolerance::calibrated()` appears only in the two golden test files and the calibrate example; no `!report.passed` assertion is ever made at the calibrated tolerance.
- **Fix:** Add a test that a documented-bad candidate fails at `calibrated()` — the repo already has the perfect fixture in history (the pre-kerning-fix `para-carlito` measured SSIM 0.2348): e.g. render `para-carlito` with kerning force-enabled, or synthetically shift the candidate 4 px horizontally, and assert `!passed`. Also a cheap floor: `assert!(CALIBRATED_MIN_SSIM >= 0.5 && CALIBRATED_MAX_DELTA_E <= 15.0)` pinned to the calibration record.

### CF-2 — `is_available_is_callable` asserts nothing
- **File:** `appthere-conformance/src/schema/xmllint_tests.rs:51-55`
- **Category:** A · **Confidence:** HIGH
- **Defect:** `let _ = XmllintValidator::is_available();` — no assertion; passes for any implementation including one that panics-free returns garbage.
- **Fix:** Cross-check coherence: `assert_eq!(XmllintValidator::is_available(), XmllintValidator::new().is_ok())` — true in every environment.

### CF-3 — `severity_totals_match_the_plan` is a partition tautology; the planned totals in its comment are never asserted
- **File:** `appthere-conformance/src/corpus/catalog/mod.rs:111-120`
- **Category:** A · **Confidence:** HIGH
- **Defect:** The test sums `cases_with_severity(s)` over all three enum variants and compares to `all_cases().len()` — true by construction for *any* assignment of severities (every case has exactly one `Severity`). The doc comment cites "22 P0 / 82 P1 / 47 P2", but no per-severity count is asserted, so any re-labelling (every case silently downgraded to P2) passes.
- **Evidence:**
  ```rust
  let total: usize = [P0, P1, P2].iter().map(|s| cases_with_severity(s).len()).sum();
  assert_eq!(total, all_cases().len());
  ```
- **Fix:** Assert the three per-severity counts as literals, exactly as `counts_match_plan_totals` (the neighbouring, well-formed test) does for formats.

### CF-4 — `feature_grouping_partitions_the_catalog` is near-vacuous
- **File:** `appthere-conformance/src/corpus/catalog/mod.rs:122-126`
- **Category:** A · **Confidence:** MEDIUM
- **Defect:** Sum of group sizes equals total — holds for any `by_feature` that is a grouping; it can only catch a drop/duplicate bug in the grouping helper, not any property of the catalog. Fine as a helper sanity check, but its name promises catalog knowledge it doesn't have.
- **Fix:** Assert at least one known feature key exists with an expected count, or rename.

### CF-5 — xmllint suite silently self-skips off-CI
- **File:** `appthere-conformance/src/schema/xmllint_tests.rs:30-38` (`validator_or_skip`)
- **Category:** B (environment-gated vacuity) · **Confidence:** HIGH
- **Defect:** 4 of 6 tests return green with only an `eprintln!` when `xmllint` is absent; nothing asserts that at least one environment actually runs them (contrast `raster_tests.rs`, which hard-`expect`s pdftoppm and so fails loudly). A CI image change that drops libxml2 would silently disable the schema axis.
- **Fix:** Gate on an env var the CI job sets (`if std::env::var("CI").is_ok() { panic!("xmllint required in CI") }`) inside `validator_or_skip`, keeping the local skip.

Healthy: the perceptual differ suite is strong — `localized_defect_is_not_averaged_away` proves the worst-region semantics against the mean it replaced, CIEDE2000 is pinned to Sharma et al. published triples with a symmetry check, sRGB→Lab anchors, heatmap hot/cold discrimination; roundtrip/model/sheet diff tests each pair "caught with a path" with both-sides evidence; manifest tests assert on-disk existence.

---

## loki-acid

### LA-1 — `golden_pages_match_within_ssim_threshold` is vacuous in this repo
- **File:** `loki-acid/tests/golden_pixel.rs:21-73`
- **Category:** A (documented deferral) · **Confidence:** HIGH
- **Defect:** `loki-acid/renders/` is empty (verified) — every golden (only `goldens/acid_odt/` exists) is skipped for want of a matching render, `compared == 0`, and the test returns green after an eprintln. The SSIM threshold 0.98 has therefore never gated anything.
- **Fix:** As with LRC-1: `#[ignore]` until the render tree is producible in-repo — or better, now that `loki-render-cpu::render_page` exists, generate the renders in the test itself instead of expecting a pre-populated tree (the external-step design predates the CPU renderer; `golden.rs`'s own docs say "until a rasteriser produces the Loki renders headlessly", which is no longer true).

### LA-2 — the page-count "canary" pins no count
- **Files:** `loki-acid/tests/structural.rs:43-58, 95-105`; `loki-acid/src/report.rs`
- **Category:** C · **Confidence:** HIGH
- **Defect:** The plan names *page-count drift* as a cheap canary, but no test records an expected page count for any fixture: `word_processing_fixtures_paginate` asserts `!pages.is_empty()`, `aggregate_report_covers_all_fixtures` asserts `page_count.is_some()`. A layout regression that doubles or halves pagination of `acid_docx`/`acid_odt` passes the entire suite.
- **Evidence:** grep for any literal expected page count over loki-acid: none. The report type carries `page_count` precisely so it can be compared, and nothing compares it.
- **Fix:** Capture today's counts **with independent verification** (open each fixture in Word/LibreOffice once, record both counts in the test with a comment naming the reference app and date), then `assert_eq!(layout.pages.len(), EXPECTED[fixture])`. An unverified snapshot would be the exact trap the audit brief warns about; the verification note is the difference.

### LA-3 — glyph-coverage assertion cannot fail; the real check is permanently `#[ignore]`d
- **File:** `loki-acid/tests/structural.rs:54-57` and `:113-129`
- **Category:** A (the in-suite assertion) + C (the gate) · **Confidence:** HIGH
- **Defect:** The only glyph-coverage assertion that runs is `cov.notdef_glyphs <= cov.total_glyphs`, which is an invariant of the counter by construction (`scan_item` increments `total_glyphs` for every glyph and `notdef_glyphs` only alongside it — `pages.rs:88-98`); it cannot be false. The discriminating check (`no_tofu_glyphs`) is `#[ignore]`d as host-font-dependent, so "no tofu" — named a P0 canary in the crate's own docs — is never enforced anywhere automatic.
- **Fix:** The bundled-font path already makes a deterministic middle ground possible: lay out with `loki_fonts::fallback_font_blobs()` registered (as `render_smoke.rs` does) and assert zero tofu for the fixtures' Latin text — that is host-independent. Keep the strict host-font variant ignored.

### LA-4 — `missing_goldens_yields_empty` asserts current repo state
- **File:** `loki-acid/src/golden.rs:84-89`
- **Category:** B · **Confidence:** HIGH · **Severity:** low
- **Defect:** Asserts `golden_pages(Fixture::Docx).is_empty()` because "No goldens are committed" — a test of the repository's contents, not of code; it starts failing the day DOCX goldens land (and `goldens/acid_odt/` already shows the comment's premise decaying).
- **Fix:** Point it at a guaranteed-absent stem (tempdir or `"no-such-fixture"`), which `appthere-conformance`'s own `missing_golden_dir_yields_empty` already does correctly.

### LA-5 — `report_serialises_to_json` smoke
- **File:** `loki-acid/src/report.rs:97-105`
- **Category:** A · **Confidence:** HIGH · **Severity:** low
- **Defect:** Asserts only that serde output contains the substring `total_cases` — tests the derive, not the crate.
- **Fix:** Round-trip (`from_str` back) or drop; near-zero value either way.

Healthy: `acid2_word_valid.rs` is excellent (both Word-vs-tolerant asymmetries, cross-part separator check with named failure history); `fixtures.rs` trait-driven corpus test asserts the honest-unsupported export; structural import tests cover both supported and documented-unsupported polarities.

---

## loki-bench

### LB-1 — `measure_runs_the_workload_and_returns_well_formed_stats` is vacuous without the dhat allocator
- **File:** `loki-bench/src/memory_tests.rs:10-23`
- **Category:** B · **Confidence:** HIGH
- **Defect:** Under `cargo test` the dhat global allocator is not installed, all four stats are 0, and both invariant assertions reduce to `0 <= 0`; the only thing the test can fail on is the workload not running. The name promises "well_formed_stats"; the module doc admits the nonzero-signal proof lives in the `portable_alloc` **bench** (`benches/portable_alloc.rs:32` does assert nonzero) — which `cargo test` never executes, so broken dhat wiring reaches CI green.
- **Fix:** Add one `#[test]` in a target that installs the allocator — Cargo supports per-test-binary globals via an integration test with `loki_bench::dhat_global_allocator!()` — asserting `measure(|| {vec![0u8; 4096];}).total_bytes >= 4096`. Same gap applies to `leak::residual_after` (only `classify_leak` is tested).

### LB-2 — timing controls' ordering property is untested
- **File:** `loki-bench/src/timing.rs:96-141` (`compare`, `sweep`), tests at `:221-235`
- **Category:** C · **Confidence:** HIGH
- **Defect:** The module exists (per its own ADR L08-035 header) to make "warm both before timing either" structurally guaranteed, but the only test covers `verdict` string formatting. Reordering `compare` to warm-then-time each side in sequence — the exact defect that produced the retracted 80% finding — fails no test.
- **Evidence:** `mod tests` contains one test, `small_differences_report_parity_rather_than_a_percentage`.
- **Fix:** The ordering is observable without a clock: pass closures that append to a shared `Vec<&str>` and assert the trace is `[warm_a, warm_b, time_a, time_b, …]` (for `sweep`: all warms strictly before all timed passes). Cheap, and it pins the module's entire reason for existing.

### LB-3 — `default_residual_is_zero`
- **File:** `loki-bench/src/leak_tests.rs:43-52`
- **Category:** A · **Confidence:** HIGH · **Severity:** low
- **Defect:** Asserts that `#[derive(Default)]` produces zeros — cannot fail while the derive exists; tests the compiler.
- **Fix:** Delete, or fold into a test of `residual_after` when LB-1's allocator-installed target lands.

Healthy: baseline diff suite covers every `DeltaStatus` variant including the 0→nonzero `+INF` acceptance case and jitter tolerance; parity trigger tests pin sibling-crate confusion (`vello_cpu` vs `vello`); rss parser covers the prefix-collision trap; budget boundary (`check(1_000, 1_000)` within) is exact; axis mapping pinned per variant on both sides.

---

## Per-crate statistics

| Crate | Test files read | Tests examined | A | B | C | Notes |
|---|---|---|---|---|---|---|
| appthere-canvas | 9 | 69 | 1 | 1 (doc-level) | 1 | AC-1..3 |
| loki-renderer | 10 | 52 | 1 (low) | 2 | 2 | LR-1..5 |
| loki-vello | 5 | 21 | 2 | 0 | 0 | LV-1..2 (LV-2 groups 6 no-assert tests) |
| loki-render-cpu | 5 | 14 | 1 | 0 | 0 | LRC-1 (3 tests); calibrated-tolerance gap filed as CF-1 |
| appthere-conformance | 10 | 51 | 3 | 1 | 1 | CF-1..5 |
| loki-acid | 8 | 22 | 3 | 1 | 1 | LA-1..5 |
| loki-bench | 8 | 30 | 1 (low) | 1 | 1 | LB-1..3 (grep's 31 counted a doc-comment `#[test]`) |
| **Total** | **55** | **259** | **12** | **6** | **6** | |

## Healthy areas

The dominant impression is far above baseline. The residency stack
(appthere-canvas planner/budget + loki-renderer wiring) is close to a model
suite: independent literals from the S0.2 spike, explicit polarity tests that
exist to stop sibling tests being satisfied by constants, fixture
preconditions asserted inside tests, sweeps that drive the *production*
planner rather than a model of it, and `Arc::ptr_eq` identity assertions where
"recomputed to look the same" is the failure mode. loki-render-cpu's hatch and
data-URI tests each carry a genuine inversion and a control. The ODT visual
axis is properly independent (LibreOffice-generated goldens with committed
provenance and a written calibration record). loki-bench's pure verdict logic
(baseline diff, leak classifier, parity trigger) is thoroughly and honestly
tested.

The weaknesses concentrate in exactly the places the audit brief predicted:
**pixel/golden tests that quietly compare nothing on a clean checkout**
(LV-1, LRC-1, LA-1 — three separate implementations of "skip if no goldens",
none visible in the test summary), **canaries with no pinned canary value**
(LA-2, LA-3), **no floor under the calibrated tolerance** (CF-1), and
**measurement plumbing whose only real assertions live in bench targets that
`cargo test` never runs** (LB-1, AC-2).
