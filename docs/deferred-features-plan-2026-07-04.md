<!--
SPDX-License-Identifier: Apache-2.0
-->

# Deferred-Features Remediation Plan (2026-07-04)

| | |
|---|---|
| **Status** | Plan — no code changes in this document's pass. |
| **Source** | [`docs/deferred-features-audit-2026-07-04.md`](deferred-features-audit-2026-07-04.md) — every task below cites its audit section. |
| **Already done** | The audit's §8 documentation-hygiene actions (S-1…S-9 stale-doc fixes) were applied on 2026-07-04 and are **not** in this plan. Verified: no stale `TODO(editing)` / `position: absolute` COMPAT notes remain in code, and the `CLAUDE.md` rows are reconciled. |
| **What this plan covers** | The genuine, correctly-documented deferrals in audit §2–§7, sorted into: work we can and should do (Phases 0–7), upstream-gated items we can only watch (§ Watch list), and deliberate post-MVP scope we will not schedule (§ Out of scope). |

## How to read this plan

- **Priority**: P0 = correctness/data-integrity or unblocks other phases; P1 = committed spec work left unbuilt; P2 = quality/perf/fidelity backlog; P3 = polish.
- **Effort**: S ≤ 1 day, M ≤ 1 week, L > 1 week (single engineer, familiar with the crate).
- Phases are ordered by priority but are largely **independent workstreams** — they can proceed in parallel except where a dependency is called out.
- Every task keeps the repo conventions: root-cause fixes only, 300-line ceiling (ratcheted), `cargo fmt` + `clippy -D warnings`, and a `docs/fidelity-status.md` update for any layout/rendering/import-export change.

---

## Phase 0 — Verification pass: re-drive the unverified findings (P0, S)

The audit explicitly did **not** re-drive the app-layer findings F1–F7 from
`audit-2026-06-10` (presentation tab-switch edit loss, no-op delete/copy, dead
retier channels, no Save-As) — they are "likely-open pending a focused check"
(audit §5, closing note). Planning fixes against unverified findings risks the
exact stale-doc failure mode the audit was written to catch.

| Task | Detail | Exit criterion |
|---|---|---|
| 0.1 | ✅ **Done 2026-07-04** — F1–F7 re-driven; verdicts in the audit's new §9 addendum. Headline: F1/F2/F4 resolved, F3 largely resolved, F5 resolved-by-removal, F6/F7 partial. | Each of F1–F7 has a verified status with `file:line` evidence. |
| 0.2 | ✅ **Done 2026-07-04** — confirmed-open subset folded into Phases 4/6 and the Watch list (see 4b.6/4b.7, 4c.5, 6.7 below). | Phase 4 backlog updated. |

---

## Phase 1 — CRDT round-trip integrity (P0, M)

Data that survives import but is silently degraded by the Loro bridge is the
closest thing to data loss in the suite. These are all code-confirmed in audit
§6 and §2 (`loro-bridge` topic).

| Task | Source | Detail | Effort |
|---|---|---|---|
| 1.1 | §6 | ✅ **Done 2026-07-04** — structured tab-stop codec (`loro_bridge/decode.rs`) + reader; `bridge_tab_stops_roundtrip`. | S–M |
| 1.2 | §6 | ✅ **Done 2026-07-04** — total `DocumentColor` codec (`loro_bridge/color_codec.rs`, Rgb/Cmyk/Theme/Transparent) + reader; `bridge_para_background_color_roundtrip`. | S |
| 1.3 | §6 | ✅ **Done 2026-07-04** — native mappings for `BulletList`/`OrderedList`/`BlockQuote`/`Div`/`Figure` (`loro_bridge/containers.rs`, table.rs pattern); `loro_bridge_container_tests.rs`. `DefinitionList` + inline fields/math stay opaque. | M |
| 1.4 | §2 | ✅ **Done 2026-07-04** — char colors use the total `DocumentColor` codec (Theme/Cmyk/Transparent survive, mark + block-map paths); comment/bookmark anchors preserved as `OBJECT_REPLACEMENT_CHAR` snapshot marks; `Quoted` quote-type and `Span` attrs carried as range marks with recursive child writes; `loro_bridge_inline_tail_tests.rs`. Remaining TODO(loro-bridge): non-Rgb *border* colors (colon-format migration), `Cite` metadata, structural-table CRDT semantics. | M |
| 1.5 | §2, §5 | ✅ **Done 2026-07-04** — `loro_bridge::compact` (`compact_in_place` / `compact_history`) wired at the save point in loki-text (`editor_compact.rs`; ribbon Save unified into the Ctrl+S handler). Bench acceptance passed: 5 000 keystrokes = 19 208 ops uncompacted → 1 op compacted, asserted in `leak_loro_history`. On-device long-session validation still pending (BM-14). | M |

**Exit criteria**: `metadata_round_trip.rs`-style tests pass for 1.1–1.3; the
`leak_loro_history` bench shows bounded history; the two §6 tech-debt rows and
the bridge-stubs row in `CLAUDE.md` are updated to DONE.

---

## Phase 2 — The one actionable dependency patch: `loki-file-access` (P0, S)

Audit §3: of the 6 `[patch.crates-io]` entries, five are gated on upstream
Dioxus/Blitz releases (watch list), but **`loki-file-access` 0.1.2 is
same-team-owned** (`appthere/loki-file-access`) and its removal condition is
entirely in our hands.

| Task | Detail |
|---|---|
| 2.1 | ✅ **Done 2026-07-05** — full patch content (Android NativeActivity fixes, Java shims + dexing `build.rs`, insets/IME bridges, `token.delete()`/`copy_bytes_to()`) upstreamed to `appthere/loki-file-access` as **0.1.3**, commit `d2b7bc5` fast-forwarded to `main` (fix branch `claude/upstream-android-nativeactivity-fixes` also pushed). 43 tests + clippy clean upstream. |
| 2.2 | ✅ **Done 2026-07-05** — `[patch]` entry and `patches/loki-file-access/` removed; the `branch = "main"` git dependency resolves to 0.1.3 directly. The three Android build scripts now resolve the Java-shim directory from `cargo metadata` instead of the deleted patch path. Registry publication remains optional (the workspace dep is git, not registry) — noted in `docs/patches.md` "Removed patches". |

**Exit criterion**: ✅ met — `cargo check --workspace` with no `loki-file-access`
patch and zero `Patch ... was not used` warnings.

---

## Phase 3 — Conformance foundation: build the Spec 02 "resolved-as-decision" items (P1, L)

Audit §1 note + §4: several Spec 02 items carry a "✅ Resolved" *decision* but
were never built — verified again 2026-07-04 (no Gelasio face in
`loki-fonts/fonts/`, no vendored schemas, zero goldens). This phase turns
decisions into artifacts, in dependency order. It also **unblocks Spec 06
BM-3** (render-cost proxy) and the ACID headless-raster registry item.

| Task | Spec item | Detail | Effort |
|---|---|---|---|
| 3.1 | B-10 | ✅ **Done 2026-07-05** — Gelasio ×4 faces bundled (OFL, full coverage reconstructed from fontsource subsets), Georgia→Gelasio mapping, dedicated substitution suite (`loki-layout/tests/font_substitution_suite.rs`). | S |
| 3.2 | B-6 | ✅ **Done 2026-07-05** — ISO 29500-4:2016 Transitional + mce, ECMA-376 Part-2 OPC, OASIS ODF 1.3 RNG + MathML3 vendored with PROVENANCE (sha256); real DOCX/ODT exports schema-validated incl. malformed-part negative tests. Tails: Strict XSDs, Dublin Core imports for core.xml (documented in `schemas/README.md`). | M |
| 3.3 | B-1 | ✅ **Done 2026-07-05** — new `loki-render-cpu` crate: `vello_cpu` (=0.0.9) rasterizes the same `PositionedItem` stream the GPU path paints, headless, byte-deterministic (M5 acceptance smoke tests). TODO(conformance-render): decode image items (grey placeholder today). | M |
| 3.4 | B-5 | ✅ **Done 2026-07-05** — `appthere_conformance::raster::PdfRasterizer` (pdftoppm pinned flags @ `CONFORMANCE_DPI` 144, version captured, byte-determinism tested). | S |
| 3.5 | B-2, B-3, B-4 | ✅ **Done 2026-07-05** — SSIM+CIEDE2000 worst-region differ with heatmaps (`golden/diff.rs`, Sharma reference pairs verified); 3 ODF goldens committed with GENERATION metadata (`scripts/generate-odf-goldens.sh`); calibration record `goldens/CALIBRATION.md` → `Tolerance::calibrated()` {0.60, 10.0}. **The calibration pass found and quantified fidelity gap #23 (kerning)** — pinned as the `para-carlito` expected-failure canary. OOXML goldens remain the manual Windows/Word COM procedure (§7.2). | M |
| 3.6 | B-11 | ✅ **Done 2026-07-05** — all three axes run as cargo tests in the existing `build-and-test` job (xmllint + pdftoppm installed); schema + round-trip are hard gates, visual is advisory-by-construction (known divergence pinned; hardens when kerning lands + recalibration). | S |
| 3.7 | B-8, B-9 | **Deferred** — shared `Fixture`/`Consumer` trait extraction from `loki-acid` and the 141-case corpus reorg remain open (the new fixtures/goldens live under `appthere-conformance/{fixtures,goldens}` as the seed of that layout). ODP/ODG/PPTX importers stay gated on the unbuilt ACID PPTX generator (§5.1). | M |

**Exit criterion**: ✅ met for the built scope — CI runs schema, round-trip,
and visual-golden passes on every PR; Spec 02 rows B-1…B-6, B-10, B-11 are
*built*, B-7 was already largely done, B-8/B-9 remain the tracked tail.

---

## Phase 4 — Editor completion: Spec 04/05 gaps + verified UX TODOs (P1, L)

The audit calls Spec 04 "the least-complete 'shipped' spec" (§4). This phase
groups the user-visible editor work: spec milestones first, then the §2 UX
TODOs (merged with whatever Phase 0 confirms from F1–F7).

### 4a. Spec milestones (model/architecture-gated first)

| Task | Source | Detail | Effort |
|---|---|---|---|
| 4a.1 | Spec 04 M3 | Width-driven ribbon collapse cascade: condensed variant, overflow menu, per-group priority, hysteresis. | L |
| 4a.2 | Spec 04 M5 | Layout/References/Review ribbon tabs + `selected_object` contextual-tab signal (only 3 non-contextual tabs exist). | L |
| 4a.3 | Spec 05 | **Page** style family (`page_styles` catalog per ADR-0012) and **Table** family (`TableProps` conditional/banding regions); character-style editing form; per-family non-paragraph `Default` sources; Compact-tree breadcrumb (M7). | L |
| 4a.4 | Spec 03 | Metadata-panel label stacking <250 px (R-13g); responsive doc type-scale (M4); real `Viewport.zoom`. | M |
| 4a.5 | Spec 04 M6 | Touch posture + cursor-into-new-cell after insert. | M |

### 4b. Editing-core TODOs (§2)

| Task | Topic | Detail | Effort |
|---|---|---|---|
| 4b.1 | `3b-3` | Finish cross-page navigation: left/right at page edges; `page_index` recompute after split/merge (currently `None`). | M |
| 4b.2 | `formatting` | Multi-block-selection formatting (today clamped to the focus paragraph) — `editor_formatting.rs:106`. | M |
| 4b.3 | `undo-dirty` | Saved-vs-undo-stack clean tracking. **Depends on Save existing** — sequence after the Save/Save-As outcome of Phase 0 (F-findings). Also the natural moment to do Spec 01's typed `SaveError` residual. | M |
| 4b.4 | `nested-nav` | Sibling-path navigation inside cell/note bodies (`navigation.rs:138,174`). | S |
| 4b.5 | `rotated-cell-editing` | Editing data for rotated table cells (`flow.rs:1676`) — read-only today. **Note:** `flow.rs` is a top ceiling offender; split it (Phase 7) before or with this change. | M |
| 4b.6 | F3c + F1 residual (audit §9) | Dirty-work protection: confirm-before-close for dirty tabs in loki-text (`shell.rs:101-145` discards the stashed session silently) and save-prompt/retention on presentation tab switch (`editor_inner.rs:50-57`). | S–M |
| 4b.7 | F6c + F6f (audit §9) | Selection-aware typing/Backspace (replace active selection), clipboard (copy/cut/paste), and move save/load I/O off the UI thread (`editor_ribbon.rs:93`, `editor_load.rs:56-101`). | M |

### 4c. Shell/UX polish TODOs (§2) — batchable

| Task | Topics | Detail | Effort |
|---|---|---|---|
| 4c.1 | `ux` | Confirm-before-delete dialog in all three apps' Home (delete is immediate today). Small, user-data-protecting — do first in 4c. | S |
| 4c.2 | `a11y` | Expand invisible status-bar touch targets to `TOUCH_MIN` (WCAG 2.5.8 is a stated project convention). | S |
| 4c.3 | `title-edit`, `browse-templates`, `tabs` | Inline-editable title; template-browser dialog; tab-driven navigation + blank-doc. | M |
| 4c.4 | `icons`, `ribbon`, `theme`, `platform`, `font` | Real Tabler/SVG icons over emoji; ribbon separator variant; **light-theme tokens** (currently Dark-only); macOS traffic-light region / real OS check; verify bundled UI fonts registered. | M |
| 4c.5 | F6a/F6d/F7a/F7b/F7c (audit §9) | Extract recent-file rows into child components (hooks-in-loops); wire zoom controls (all 3 apps) + spreadsheet ribbon tab-select/collapse; adopt `use_breakpoint()` in `AtHomeTab`; stable list keys + `active_slide_idx` fix-up on slide delete; word count in the status bar. | M |

---

## Phase 5 — Rendering & format fidelity backlog (P2, ongoing)

Locally-actionable fidelity items from §2 and §5 (upstream-gated ones are in
the Watch list). Every task here must update `docs/fidelity-status.md`.

| Task | Source | Detail | Effort |
|---|---|---|---|
| 5.1 | `tab-default` | Honour `DocumentSettings.default_tab_stop_pt` instead of the hardcoded 36 pt (`para.rs:648`). Pairs naturally with 1.1 (`tab_stops` round-trip). | S |
| 5.2 | `underline-style` / `strikethrough-style` | Double/Dotted/Dash/Wave underline, double strikethrough (all render Single today). Decoration geometry is ours (drawn in `loki-vello`), not Parley-gated. | M |
| 5.3 | `spell-baseline` | Tighten squiggle to the run underline offset (`para.rs:1619`). | S |
| 5.4 | `list-picture-bullet` | Picture bullets (fallback is `•`) — image plumbing already exists for block images. | M |
| 5.5 | `pdf-rotate` | Rotation transform in PDF export (`pdf/src/page.rs:83`); unlocks the "PDF clip/rotate paint" registry row. | M |
| 5.6 | gap #12 / `floating-image` | External-URL images render a grey placeholder (`loki-vello/src/image.rs:34`) + detect "floating" class for inline images (`resolve.rs:705`). | M |
| 5.7 | `odf-master-page` | ODF master-page transitions (`odf/reader/styles.rs:200`); pairs with the `style:default-style` registry row. | M |
| 5.8 | `omml` | OMML↔MathML: delimiters, n-ary, matrices, accents (`docx/omml/mod.rs:20`). | L |
| 5.9 | gaps #23–#30 tail | ~~Kerning~~ (✅ **#23 done 2026-07-05**: root-caused by the Phase 3 calibration pass — loki kerned unconditionally while Word/LO default off; `CharProps.kerning` now drives a shaper feature toggle with reference-matching default, regression-locked, all three visual goldens green), orphan/widow control, `border_between`, DocxSettings, content controls, language tags — schedule individually from the fidelity registry; orphan/widow is the highest-value (visible in any multi-page doc). | L (aggregate) |
| 5.10 | registry | Page/column geometry set: even/odd blank pages, unequal column widths, column height balancing; PDF font subsetting + ICC/CMYK; EPUB math/fields/comments. | L (aggregate) |
| 5.11 | `link-click` | Interactive hyperlink hit-testing (visual hint only today) — spans layout (`resolve.rs:689`, `items.rs:125`, `para.rs:203`) and renderer (`scene.rs:519`). | M |

---

## Phase 6 — Performance & memory (P2, M–L)

From audit §5 (memory-audit + perf tails) and §2. Re-measure with the
`loki-bench` harness before and after each item — do not fix unverified perf
findings (P-3/P-5/P-6 were "not re-driven").

| Task | Source | Detail |
|---|---|---|
| 6.1 | memory F3 | Drop preserved layout for inactive tabs (`sessions.rs:39` retains `Arc<PaginatedLayout>`); coordinate with Spec 06 BM-8 (inactive-tab layout retention policy) so one design serves both. |
| 6.2 | memory F5 | Share the render `FontDataCache` across tiles (per-tile `page_paint_source.rs:53` vs shared `DocPageSource`); same item as Spec 06 BM-9 (per-tile font-byte dedup). |
| 6.3 | `split-optimise` | Y-range item filter to avoid GPU clipping (Option B; Option A shipped) — `para.rs:409`. |
| 6.4 | `partial-render` | Viewport clipping / direct `node.scroll_offset` (`scene.rs:148`, `editor_pointer.rs:139`) — partially gated on the vendored blitz-dom scroll API; do the locally-possible part. |
| 6.5 | audit P-3/P-5/P-6 | Re-measure first (bench), then fix if confirmed: glyph-run scans, coarse cache invalidation, cold-path clones. |
| 6.6 | Spec 06 tails | BM-3 render-cost proxy executes once Phase 3.3 lands `vello_cpu`; GPU frame-time (`device` feature) and on-device RSS recalibration remain device-gated — keep deferred, tracked in Spec 06. |
| 6.7 | F5 residual (audit §9) | Replace the hardwired-`false` `DocumentViewProps::eq` (`document_view.rs:143-147`) with a real comparison — benign today (PageTile eq caps the cost) but pure over-render. |

---

## Phase 7 — Code-quality debt (P2/P3, ongoing ratchet)

| Task | Source | Detail |
|---|---|---|
| 7.1 | Q-1 / §6 | **300-line split pass**: burn down the 35-file baseline, starting with the files other phases must touch (`para.rs` 1979, `flow.rs` 1953 — both also carry Phase 4/5 TODOs) so the split happens *before* feature work grows them further. Use the two established techniques (inline-test extraction, directory split); update the baseline with `--update` per split. |
| 7.2 | Q-2 | App-shell duplication: extract the common per-app `routes/` + `shell.rs` scaffolding into `loki-app-shell` (it already hosts `android_main!` and `SpellService`). |
| 7.3 | Q-3/Q-4 | Writer error-swallows (301 `let _ =`) and the ~100 `#[allow]` (incl. 32 `dead_code` in OOXML): ratchet, don't big-bang — add a CI count-ratchet script (same pattern as the file ceiling) and reduce opportunistically when touching a file. |
| 7.4 | audit S-1b/S-2/S-3/S-5 | Security tails: nested-table drop, dimension clamp, UTF-16 odd byte, XXE comment. Small, parser-local; batch into one hardening pass with fuzz-style regression tests. |
| 7.5 | T-2/T-3/T-5 | Test tails: ODT export coverage, per-case DOCX/XLSX round-trips, hard PPTX cases — grow alongside Phase 3's conformance corpus rather than as a separate effort. |
| 7.6 | Spec 01 residuals | `clippy::pedantic` + allow-list, `no_hardcoded_layout_dims` dylint, `cargo udeps` dead-`pub` sweep, Android target build verification in CI. Deliberate residuals; schedule after 7.1 has reduced churn. |

---

## Watch list — upstream-gated, not schedulable (re-check on every dependency bump)

No local work can close these; the action is to **re-verify the removal
condition whenever the pinned dependency moves** (per the "Upgrading Dioxus"
procedure in `docs/patches.md`).

- **The five Dioxus/Blitz patches** (`dioxus-native`, `dioxus-native-dom`, `blitz-shell`, `blitz-net`, `blitz-dom`) — each has its removal condition in `docs/patches.md`; none met as of 0.7.9 / 0.2.x.
- **Parley**: native `BaselineShift` (super/sub is already visually correct via the manual offset — S-3), bidi/RTL direction API (gap #19), inline image boxes (`inline-image-flow`).
- **Vello**: blur primitive for true soft text shadow (`shadow` — hard offset copy today).
- **Blitz CSS**: `white-space: nowrap`, `text-overflow: ellipsis`, `:hover`, `scrollbar-width`, SVG rendering, `position: fixed` (collapses to `absolute`) — runtime-verify each on every Blitz bump and update the `CLAUDE.md` confirmed/unconfirmed lists.
- **Platform quirks (permanent)**: Mali-G715 Vulkan device-loss workarounds, Android 16 double `ANativeActivity_onCreate`, Word/OOXML file quirks — documented COMPAT, never removable.
- **Patch-tree fixes queued for the next re-vendor** (audit §9 F7e/F7f): strip the `[LOKI/head]` / `println!` / `dbg!` debug leftovers from the vendored patches, and fix the `buttons ^= Main` XOR on touch end/cancel (`patches/blitz-shell/src/window.rs:1133`, should be `&= !Main`) — batch these with the next Dioxus/Blitz patch re-vendor rather than churning the vendored tree now.
- **Headless/server deferrals with in-code TODOs**: `TODO(headless-c025)` (apalis job queue), `TODO(headless-c021/c022/c023-discovery/c027/c028)`, `TODO(kms)`, `TODO(ws-membership)` — deliberate spec deferrals (ADR C021–C028); schedule when the server milestone that needs them is planned, not before.

## Out of scope — deliberate post-MVP (do not schedule)

Audit §7: Loki Calc / Loki Slides post-MVP items (virtualization >500×52,
richer formulas, charts, PPTX image/group export, masters/layouts, ODP import,
shape editing, etc.) remain governed by
`docs/mvp-scope-spreadsheet-presentation-2026-06-13.md`. Two exceptions worth
pulling forward when either app is next touched, because they violate suite
conventions rather than MVP scope:

1. **i18n bypass** — both apps hardcode user-visible strings, against the "never hardcode" rule; migrate to `fl!()` opportunistically.
2. **Zero tests** — add smoke tests for whatever Phase 0 confirms from F1/F2 (edit loss, no-op delete/copy) before fixing them.

---

## Suggested sequencing

```
Now (parallel):   Phase 0 (verify F1–F7)   Phase 2 (loki-file-access)   Task 7.1 (split para.rs/flow.rs)
Next:             Phase 1 (CRDT integrity) Phase 3 (conformance foundation)
Then:             Phase 4 (editor completion, informed by Phase 0)
Ongoing ratchet:  Phase 5 (fidelity), Phase 6 (perf, bench-gated), Phase 7 (quality)
Every dep bump:   Watch list re-verification
```

Rationale for the front of the queue: Phase 0 is cheap and de-risks Phase 4;
Phase 2 is the only patch we fully control and shrinks the patch surface;
splitting `para.rs`/`flow.rs` first prevents every later phase that touches
them (5.1–5.4, 4b.5, 6.3) from fighting the ceiling ratchet; Phase 1 protects
user data; Phase 3 builds the verification infrastructure that keeps Phases 4–5
honest.
