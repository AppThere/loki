<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 06 — Benchmarking & Continuous Memory Tracking: Audit Report

| | |
|---|---|
| **Status** | Audit complete; **no code changes**. Ground-truth inventory + finding register (**BM-1 … BM-14**) + triaged M1–M6 readiness below. |
| **Method** | Audit-first per Spec 06 §1/§4.1: inventory what is measurable today, where the caches are and whether they are bounded, and how Loro history behaves — before building the harness. |
| **Companion** | [spec-06-benchmarking-and-memory-tracking.md](spec-06-benchmarking-and-memory-tracking.md) (the design spec) |
| **Precedent** | Same audit-then-triage flow as [spec-01](spec-01-audit-report.md) / [spec-02](spec-02-conformance-inventory.md) / [spec-05](spec-05-style-audit.md). |
| **Primary source** | [docs/memory-audit-2026-06-12.md](../memory-audit-2026-06-12.md) — a prior code-level RAM audit (Findings 1–7) this report builds on. |

This report establishes ground truth. It makes **no code changes** — implementation
waits for triage. The headline deliverables are the **measurability inventory**
(§1), the **cache-bounding table** (§2), the **Loro-history verdict** (§3), and one
**upstream-dependency correction** (BM-3: the portable `vello_cpu` render proxy the
spec leans on does not exist yet).

---

## 1. Measurability inventory — what can be benched today

| Capability | State today | Gap for Spec 06 |
|---|---|---|
| Timed micro-benches | `loki-layout` has Criterion wired (`criterion = "0.8"`, 2 targets: `layout_scaling`, `edit_path`) | Only layout is covered; no IO/export/style benches; Criterion is not a workspace-wide dev-dep |
| Open-latency bench | `loki-acid/examples/load_bench.rs` — hand-rolled `Instant` timing of the open→layout-ready pipeline, with **cold-open vs warm-reopen** stages; `relayout_bench.rs` alongside | Ad-hoc `eprintln!` reporting, not Criterion; example, not a tracked bench |
| On-device open tracing | `loki_text::open` tracing spans on the real editor open path (`editor_load` / `editing::state`) | Spans exist; nothing captures/diffs them |
| Allocation profiling (dhat) | **Absent entirely** — no `dhat` dependency anywhere in the workspace | The entire continuous-memory backbone (§7/D1) must be added from scratch |
| Peak-RSS measurement | **Absent**; `log_memory_counters` logs Loro op/change counters, not RSS | M5 device work; needs `/proc` (Linux) + platform API (macOS) |
| Scale corpus | **Absent**; only Spec 02 feature fixtures + `acid_docx.docx` exist | The 4-tier scale corpus (§8) must be generated |
| Root bench harness | **Absent**; benches are scattered per-crate | M1: the two-axis harness at the monorepo root (`loki-acid` is the closest existing shared harness and is a workspace member) |

**Verdict:** the raw signals exist in pieces (Criterion in one crate, a hand-rolled
open bench, on-device tracing spans), but there is **no shared harness, no dhat, no
baseline, and no scale corpus**. Spec 06 is a greenfield build on top of scattered
precedent, not a wiring-up of existing infrastructure.

---

## 2. Cache inventory & bounding (the §7 leak-detection targets)

| Cache | Location | Bounded? | Notes |
|---|---|---|---|
| Paragraph shaping cache | `loki-layout` `ParaCache` (`para_cache.rs`) | ✅ **Yes** | Two-generation approximate-LRU, `CACHE_CAP = 2048` × 2 gens; rotates on fill; `clear()` on document load (`clear_paragraph_cache`). Memory audit Finding 4 (**Fixed**). |
| GPU page-texture tiers | `loki-renderer` (`CacheTier` Hot/Warm/Cold, `assign_tier`) | ✅ **Yes (post-fix)** | Bounded to the viewport neighbourhood after Finding 1's initial-retier fix (~240 MB → ~45 MB for a 20-page doc). Device-bound (GPU); a portable bench cannot see it. |
| Page-tile virtualization | `loki-renderer/virtualize.rs` (`visible_window`) + `document_view.rs` | ✅ **Yes (now wired)** | `visible_window` restricts mounted tiles to the viewport neighbourhood with page-sized placeholders elsewhere — memory audit Finding 2 (was *Recommended*) is now **implemented**. |
| Inactive-tab preserved layout | `DocSession.paginated_layout: Arc<PaginatedLayout>` | ⚠️ **Retained** | Every inactive tab stashes its full layout (~9 MB/20pp). Finding 3 — *Recommended, not yet fixed*. A candidate memory-bench target (BM-8). |
| Per-tile font-byte cache | `LokiPageSource`'s own `FontDataCache` | ⚠️ **Duplicated** | Interned font bytes duplicated across tiles; `DocPageSource` holds a shared `font_cache` but tiles don't all route through it. Finding 5 — *Recommended, not yet fixed* (BM-9). |
| Render primitives | `loki-render-cache` | n/a | `PageSource`/`GpuTexture` primitives (gpu-feature), not an unbounded collection. |

**Verdict:** the two caches most likely to grow — the shaping cache and the GPU
texture tiers — are **bounded and proven**, and virtualization is now wired. Two
*recommended-but-unfixed* wastes remain (inactive-tab layouts, per-tile font bytes);
the pathological-tier leak benches (§7) should assert the bounded caches stay bounded
and quantify the two unfixed ones so the fix work (separate) has a number.

---

## 3. Loro history behavior — the sneakiest grower

- **The oplog never compacts.** Every keystroke appends ops to the Loro oplog, and
  deleted characters leave tombstones in the rich-text CRDT tree; **neither is ever
  compacted** (memory audit Finding 6). Resident memory grows with edit *history*
  during a long single-document session — the §7 "grows with time, not size" culprit,
  confirmed in source.
- **Undo is bounded.** `UndoManager` is capped (`max_undo_steps(100)`), so undo
  history itself is not the unbounded grower — the oplog/tombstones are (BM-7).
- **Instrumentation already exists.** `apply_mutation_and_relayout` →
  `log_memory_counters` logs throttled (1-per-64-mutations) `loro_ops` / `loro_changes`
  counters under the `loki_text::mem` target, explicitly so "`loro_ops` climbing while
  `pages`/`blocks` stay flat" confirms history is the grower. **The M4 Loro-history
  bench can read these counters directly** rather than inventing new telemetry.
- **Compaction is proposed, not done.** `TODO(loro-compaction)`: drop history before a
  frontier via `export(ExportMode::shallow_snapshot(&doc.oplog_frontiers()))` re-imported
  into a fresh `LoroDoc`, at a save/undo-horizon checkpoint. Deferred because it
  invalidates the `UndoManager` / `IncrementalReader` / live signals and needs on-device
  validation. **Out of scope for Spec 06** (measurement, not fix) — but the M4 bench is
  what will justify and later validate it.

---

## 4. Arc steady-state — the guarded invariant (Spec §2, §7)

Confirmed in source — the three resources the spec names are shared behind `Arc`, not
per-instance:

- `shared_renderer: Arc<Mutex<Option<vello::Renderer>>>` (`renderer_state.rs`, threaded
  through `page_tile.rs` / `page_paint_source.rs`).
- `paginated_layout: Option<Arc<PaginatedLayout>>` (`editing/state.rs`, `document_view.rs`,
  `sessions.rs`) — one layout shared by the editor and the paint path (the
  "single canonical layout").
- `shared_font_resources: Arc<Mutex<FontResources>>` (`editing/state.rs`).

**M3's steady-state guard is well-founded:** the invariant is real and localizable, so
a deliberate regression (cloning a `Renderer` instead of cloning the `Arc`) is a
detectable allocation-count delta. **Caveat (BM-14):** the "~2.83 GB → ~750 MB" and
">3 GB idle" figures in the spec and the memory audit are **code-level estimates, not
profiled** (the headless environment has no GPU); the idle >3 GB grower was ultimately
Finding 7 (a per-frame idle render loop, since **Fixed**), *not* Arc duplication. M5
must **re-measure on-device** before those numbers anchor any budget.

---

## 5. Two-axis feasibility & the `vello_cpu` gap (BM-3)

The portable axis is feasible for everything **except** the render proxy:

- **Allocation metrics (dhat), model/layout/IO/export/style op-cost** — all pure-CPU,
  headless-runnable today. Portable axis is green for these.
- **`vello_cpu` render-cost proxy — dependency unmet.** Spec 02's `vello_cpu` candidate
  render path is **specced but not implemented**: `spec-02-conformance-testing.md` is
  *"Draft — pending implementation"* and states the CPU rasterizer is *"a genuinely new
  render path — Loki does not have one today"*. In `appthere-conformance`, `golden/mod.rs`
  **describes** a `vello_cpu` candidate render, but the crate's `Cargo.toml` has **no
  `vello`/`vello_cpu` dependency** and no render body — it is doc scaffolding only. The
  only `vello_cpu` in the tree is `anyrender_vello_cpu` inside `patches/dioxus-native`
  (Blitz's own softbuffer path), not a first-party render-cost proxy.
  **Consequence:** Spec 06 M2's `vello_cpu` render-cost bench and M6's parity cadence are
  **blocked on Spec 02 landing the CPU path first.** Everything else on the portable axis
  can proceed independently; this one target waits or Spec 06 co-delivers the CPU entry
  point with Spec 02.
- **Device-bound axis** (GPU frame-time, peak RSS) is correctly un-runnable here and
  belongs on Kevin's hardware, exactly as the spec sorts it.

---

## 6. Style-resolution cost (Spec 05 target, §6)

`loki-doc-model/src/style/resolve.rs` — `resolve_para_chain` / `resolve_char_chain`
walk the parent chain on **every call**, with **no memoization** (grep for
`cache`/`memo` in `resolve.rs` is empty). This matches the spec's premise that
resolution "runs on every inspector open and every dependent recompute." It is a clean
**portable** target: pure CPU, deterministic, allocation-trackable. The M2 stressor
should scale **chain depth × style count** independently (the pathological tier's
"thousands of styles, deep inheritance chains") and watch for super-linear growth —
each resolve is O(depth), and `dependents_affected` (M4 impact) is O(styles × depth),
so a naive inspector-open over a deep, wide catalog is the thing to measure.

---

## 7. Finding register (BM-1 … BM-14)

| # | Finding | Bears on |
|---|---|---|
| **BM-1** | No root bench harness; benches scattered (Criterion in `loki-layout` only; hand-rolled `Instant` examples in `loki-acid`). | M1 |
| **BM-2** | `dhat` absent workspace-wide — the continuous-memory backbone is greenfield. | M1, M3 |
| **BM-3** | **`vello_cpu` render proxy not implemented** (Spec 02 pending); M2 proxy + M6 parity cadence are blocked upstream. | M2, M6 |
| **BM-4** | Shaping cache (`ParaCache`) is bounded (2-gen ~LRU, cap 2048, cleared on load) — assert it *stays* bounded under the pathological tier. | M4 |
| **BM-5** | GPU texture tiers + tile virtualization are bounded/wired (Findings 1 & 2) but **device-bound** — a portable bench can't observe GPU memory. | M2, M5 |
| **BM-6** | Loro oplog/tombstones never compact (Finding 6); throttled `loki_text::mem` counters already exist for the M4 bench to consume. | M4 |
| **BM-7** | `UndoManager` bounded to 100 steps — a guardable invariant, distinct from the oplog grower. | M4 |
| **BM-8** | Inactive-tab `Arc<PaginatedLayout>` retained (Finding 3, unfixed) — a memory-bench target to quantify. | M3, M4 |
| **BM-9** | Per-tile `FontDataCache` duplicates font bytes (Finding 5, unfixed) — steady-state guard should watch font-byte dedup. | M3 |
| **BM-10** | Style resolution recomputed per call, no memoization — confirmed portable target; watch super-linear on depth × count. | M2 |
| **BM-11** | Open-latency measurability exists (`load_bench` cold/warm stages + `loki_text::open` spans) — M2 open/save bench should reuse, not reinvent. | M2 |
| **BM-12** | No scale corpus — the 4-tier corpus (esp. pathological) must be generated; Spec 02 fixtures only double as the Small tier. | M2, M4 |
| **BM-13** | No committed baseline artifact or diff tooling — the "continuous" mechanism (§11) is entirely to build. | M3 |
| **BM-14** | Headline RAM figures (2.83 GB→750 MB; >3 GB idle) are **code-level estimates, not profiled**; the idle grower was Finding 7 (fixed), not Arc duplication. Re-measure on-device before budgets. | M5 |

---

## 8. Triaged M1–M6 readiness

1. **M1 — Harness + two-axis split.** *Ready.* Greenfield at the monorepo root; wire
   Criterion (already in `loki-layout`) workspace-wide and add `dhat`. Model the axis
   split on the portable/device table (§5). Reuse `loki-acid`'s fixture-loading
   precedent. Blockers: none.
2. **M2 — Portable benches.** ✅ **Shipped** (except the blocked proxy). Five
   `harness = false` allocation benches in `loki-bench/benches/`, each measuring a
   §6 target via `loki_bench::measure` over the scale corpus (`benches/support/`):
   `portable_style_resolution` (the headline — sweeps depth × chains, and the
   depth×count scaling is visible: `depth=64 chains=1000` → ~3.4 MB / 38 000
   allocs, `depth=1` resolves Local with **zero** allocations),
   `portable_layout` (cold-cache `layout_document`), `portable_model`
   (`loro_to_document` rebuild), `portable_io` (DOCX save + open), and
   `portable_export` (DOCX vs ODT emission). All run headless and produce stable
   allocation metrics. **One target still blocked:** the `vello_cpu` render-cost
   proxy waits on Spec 02 (BM-3) — deferred, not built here.
3. **M3 — Continuous memory tracking.** ✅ **Shipped.** A committed per-tier baseline
   (`loki-bench/baselines/portable.txt`, 18 curated keys — a representative metric per
   §6 target × Small/Medium/Large, plus the Arc guard metric) with a pure, tested
   diff engine (`baseline` module: parse/render round-trip + `diff` classifying
   New/Removed/Regressed/Improved/Unchanged under a jitter tolerance — counts tight,
   bytes looser for DOCX zip drift; 7 unit tests). The `baseline` bench collects the
   samples, diffs against the committed file, and prints deltas for **review** (never
   a CI failure, §11); `-- --update` rewrites the baseline intentionally. The **Arc
   steady-state guard** (`arc_steady_state` bench) asserts `Arc::clone` of a real
   shared resource allocates **zero** (audit §4); the same metric is tracked in the
   baseline, so a value-clone regression turns 0 → nonzero = `+INF` = Regressed
   (proven end-to-end and in `arc_share_replaced_by_value_clone_is_flagged`).
4. **M4 — Leak detection.** ✅ **Shipped.** A residual live-heap primitive
   (`leak` module: `residual_after` measures the heap still held after N cycles;
   pure `classify_leak` verdict — Bounded vs Leaking — 4 unit tests) drives two
   benches. `leak_detection` covers the Arc-cycle and unbounded-cache culprits:
   the real open→edit→close cycle returns to **0 B** residual (leak-free — loro
   fully frees on drop), while a **seeded** retained-document leak (1.9 MB → 124 MB
   over 1→64 cycles) and a **seeded** unbounded cache (64 KB → 4.2 MB) are both
   flagged `Leaking` — the acceptance. `leak_loro_history` measures and reports the
   third culprit: 5 000 keystrokes grow the oplog +10 000 ops (2/keystroke, ~550 B
   each) with the document length stable, quantifying Finding 6 /
   `TODO(loro-compaction)` — the yardstick a future compaction fix is validated
   against. (BM-4/BM-5's production `ParaCache`/GPU-tier boundedness stays where it
   is proven — the loki-layout `ParaCache` unit tests and, for the device tiers,
   M5; the seeded unbounded cache here proves the *detector*.)
5. **M5 — Device benches + budgets.** ✅ **Shipped (agent-runnable half); GPU
   frame-time device-gated.** Peak-RSS plumbing (BM-1) built and **verified in the
   agent**: `rss` module (Linux `/proc/self/status` `VmHWM`/`VmRSS`, pure parser +
   3 tests) drives the `device_rss` bench, which measures peak RSS per tier
   (agent: small 9.3 / medium 12.1 / large 22.9 MiB) and reviews it against the
   committed budgets (`baselines/rss_budgets.txt`). The budget mechanism (`budget`
   module: `check`/`headroom_frac` + `Budgets` parse/render, 4 tests) calibrates
   budgets at 1.5× measured peak (`-- --update`) and reviews-not-gates (§11). The
   calibration record is committed ([spec-06-calibration.md](spec-06-calibration.md)):
   measured distributions, device/date/tool versions, chosen budgets — with the
   explicit **BM-14 caveat that agent numbers under-count real devices (no GPU
   textures)** and Kevin must re-run on the Windows+RTX 3050 / MacBook A16 (adding
   the macOS/Windows RSS readers) before budgets are authoritative. **GPU
   frame-time** stays device-only (wgpu timestamp queries; `device_frame_time`
   behind the `device` feature) — the one M5 metric that cannot execute or be
   verified headless, documented for the on-device pass.
6. **M6 — Discipline + parity cadence.** *Partly blocked.* The tracked-not-gated
   discipline (§11) is pure documentation and ready; the parity cadence (§12) is blocked
   on Spec 02's `vello_cpu` path existing (BM-3) and needs a GPU, so it lands with M5's
   device work.

**Prerequisite call-outs** (so they are not discovered late): `dhat` integration and a
baseline/diff mechanism (M1/M3, greenfield); the **scale corpus** generator, especially
the pathological tier (M2/M4); and the **Spec 02 `vello_cpu` CPU render path** (BM-3),
which gates the render-cost proxy and the parity cadence.
