<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 06 — Benchmarking & Continuous Memory Tracking

| | |
|---|---|
| **Status** | Draft — pending implementation |
| **Scope** | AppThere Loki (Text); the benchmark harness is shared monorepo infrastructure |
| **Sequence** | 6 of 6 — specced last, but instrumented as a continuous concern from here on |
| **Depends on** | Spec 01 (harness location at monorepo root, ADR 0009); Spec 02 (`vello_cpu` render proxy + the CPU/GPU parity check); Spec 05 (style resolution as a named target) |
| **Feeds** | Ongoing optimization; no downstream spec |

---

## 1. Context & Motivation

RAM is expensive and getting more so, and people hold onto budget and older hardware longer because of it. The design floor is a **MacBook Neo (A16, 8 GB RAM, 2026)** — a real device a real user edits documents on while also running a browser and an OS. Loki has to be a good citizen on that machine. This isn't abstract: an earlier tiered render-cache effort already cut steady-state RAM from **~2.83 GB to ~750 MB** by sharing `vello::Renderer`, `PaginatedLayout`, and `FontResources` behind `Arc`. That win is exactly the kind of thing that silently regresses without measurement — one careless clone and it's back to gigabytes.

So this spec builds the instrumentation to (a) track performance across every operation that affects perceived responsiveness, and (b) **continuously track memory** so the 750 MB kind of win is defended, leaks are caught, and the app stays within a budget the 8 GB floor can afford. Respecting users' hardware budgets and extending device longevity through coding efficiency is the goal; measurement is how we hold ourselves to it.

Two constraints shape everything:

- **Local-only tracking.** Benchmarks are *not* CI gates. Hardware variance would make cross-machine pass/fail meaningless, so results are tracked locally and diffed against a committed baseline for human review — never a hard build failure. (Spec 01 already reserved benchmarking *out* of the gate set for this reason.)
- **No GPU in the agent environment.** Some benches (GPU frame-time, real peak RSS) can only run on actual hardware; others (allocation metrics, model/layout/IO/style cost, `vello_cpu` render cost) run anywhere. The harness must cleanly separate the two so the portable set runs headless and the device-bound set runs on Kevin's hardware.

This is **audit-first**: inventory what's measurable today, the existing caches and their bounding, and Loro history behavior, before building. References below are illustrative.

---

## 2. Relationship to Prior Specs (read first)

- **Spec 02 — `vello_cpu` is a gift here.** Adopting `vello_cpu` gives a **deterministic, hardware-independent render-cost proxy** that runs headless in the agent environment. Production frame-time is still GPU and device-bound, but `vello_cpu` render cost is a portable signal for "did rendering get more expensive." This spec also **owns the cadence** of Spec 02's CPU/GPU parity check, since the two-Vello-crate pin (`vello` 0.6 + `vello_cpu`) can drift and the parity check is what catches it.
- **Spec 05 — style resolution is a named target.** Provenance-aware resolution runs on every inspector open and every dependent recompute; on documents with deep inheritance chains and many styles it can get expensive. It is a first-class benchmark here (§6).
- **The Arc-cache win is a guarded invariant.** Memory benches must assert the shared-`Arc` steady state doesn't regress toward the old 2.83 GB behavior.
- **ADR 0009** governs layering; the harness lives at the **monorepo root** alongside the Spec 01 and Spec 02 shared infrastructure, so Presentation and Spreadsheet can bench too.

---

## 3. Goals / Non-Goals

**Goals**

- Benchmarks for every responsiveness-affecting operation: typing latency, scroll/render frame time, open/save, layout, export, and **style resolution**.
- Continuous memory tracking via a committed baseline that each run diffs, leaning on portable allocation metrics.
- Peak-RSS budgets per corpus tier, calibrated against the 8 GB floor.
- Leak detection for the known culprits: `Arc` cycles, unbounded caches, Loro history growth.
- A clean split between hardware-independent (agent-runnable) and hardware-dependent (device-only) benches.
- The CPU/GPU parity-check cadence.

**Non-Goals**

- Any CI gate. Benchmarks never fail the build (§11).
- Cross-machine result comparison (local-only; each machine tracks itself).
- The optimizations themselves. This spec *measures*; fixes are separate work informed by the measurements (though the audit may surface obvious waste, as Spec 01 does).
- Micro-optimizing anything the numbers don't flag.

---

## 4. Working Method

1. **Inventory measurability.** What can be timed/profiled today; where the caches are and whether they're bounded (LRU/eviction); whether Loro history grows unbounded or compacts.
2. **Build the harness** with the two-axis split (§5) at the monorepo root.
3. **Stand up the portable benches** first (agent-runnable): allocation metrics, model/layout/IO/style/`vello_cpu` cost.
4. **Stand up the device benches** (Kevin's hardware): GPU frame-time, real peak RSS.
5. **Calibrate budgets** (§9) against the 8 GB floor and commit the baseline.
6. **Establish the tracking discipline** (§11) and the parity cadence (§12).

Standing standards apply throughout.

---

## 5. Two Axes: Portable vs. Device-Bound

The harness sorts every bench into one of two axes, because *where it can run* and *how trustworthy its number is* differ sharply:

| Axis | Metrics | Runs where | Portability |
|------|---------|------------|-------------|
| **Portable** | heap allocation bytes/counts (dhat), model/layout/IO/style op counts, `vello_cpu` render cost | Agent, CI-like, any dev machine | High — allocation *bytes and counts* are largely hardware-independent |
| **Device-bound** | GPU frame-time, wall-clock latency, real peak RSS | Kevin's hardware (Windows+RTX 3050, MacBook A16) | Low — varies by CPU/GPU/driver |

**Key insight (D1): allocation metrics are portable in a way timing is not.** dhat reports *bytes allocated* and *allocation counts*, which barely move across machines — so **continuous memory tracking leans on allocation metrics as its primary tracked signal**, with peak RSS as a device-local reality check. This is what makes memory "continuously trackable" despite the local-only constraint: the portable signal can be diffed meaningfully over time, while timing/RSS are local reference points.

---

## 6. Benchmark Targets

Every operation the maintainer named, each tagged with its axis:

- **Typing latency** — keystroke → model update → layout → render-ready. *Portable* for the model+layout portion (op cost, allocations); *device-bound* for the wall-clock-to-pixels tail.
- **Scroll / render frame time** — *device-bound* (production GPU path); shadowed by the *portable* `vello_cpu` render-cost proxy for "did rendering get heavier."
- **Open / save** — parse/serialize + IO. *Portable* (allocations, parse op cost); IO wall-clock is *device-bound*.
- **Layout** — text layout (Parley) + pagination (`PaginatedLayout`). *Portable* (allocations, op cost).
- **Export** — DOCX/ODT/PDF emission. *Portable* (allocations, op cost).
- **Style resolution** (Spec 05) — inspector-open resolution and dependent recompute, stressed on deep inheritance chains and many styles. *Portable*. Watch for super-linear behavior as chain depth × style count grows.

Criterion drives the timed benches (it handles variance statistically); dhat wraps the allocation-tracked ones.

---

## 7. Continuous Memory Tracking (the real goal)

- **Committed baseline + diff.** A checked-in baseline of the portable memory metrics per corpus tier; each run diffs against it and surfaces deltas for review. The baseline is updated *intentionally* (committed) when a change legitimately shifts it — never silently.
- **Steady-state guard.** Assert the shared-`Arc` steady state (the 750 MB-class behavior) holds; flag regressions toward per-instance duplication of `Renderer`/`PaginatedLayout`/`FontResources`.
- **Leak detection** for the three known culprits:
  - **`Arc` cycles** — long-session tests that open, edit, and *close* a document, asserting memory returns to near-baseline. A cycle shows up as a document that never frees.
  - **Unbounded caches** — assert the render/font/layout caches actually evict (bounded LRU), not grow forever; the pathological corpus (§8) drives this.
  - **Loro history growth** — a long editing-session bench measuring whether CRDT history grows unbounded, and whether compaction/GC (if any) engages. This is the sneakiest, because it grows with session *time*, not document size.

---

## 8. The Benchmark Corpus

A **scale** corpus, distinct from Spec 02's *feature-coverage* fixtures (benchmarking stresses size; conformance stresses breadth). Four tiers:

| Tier | Shape | Flushes out |
|------|-------|-------------|
| **Small** | a page or two | baseline overhead, fixed costs |
| **Medium** | tens of pages | typical working-document behavior |
| **Large** | hundreds of pages | scaling, cache pressure |
| **Pathological** | huge tables, thousands of styles, deep inheritance chains, massive Loro history, many images | leaks, super-linear algorithms, unbounded growth |

The pathological tier is where the important bugs live — it's deliberately abusive. Where a Spec 02 fixture usefully doubles as a small input it may be reused, but the large/pathological tiers are generated for scale.

---

## 9. Budgets & Calibration

**Decision (D2): budgets are calibrated against the 8 GB floor, not guessed** — the same anti-magic-number discipline as Spec 02's threshold and Spec 01's dimension constants.

- Express a **peak-RSS budget per corpus tier** (e.g. "a large document edits under *X* MB peak RSS"), where *X* is set by measuring current behavior and targeting headroom that lets Loki coexist with an OS and a browser on 8 GB. The ~750 MB steady-state reference suggests sub-gigabyte budgets are achievable for realistic tiers.
- The calibration record — measured distributions, chosen budgets, device, date, tool versions — is committed, like Spec 02's.
- Budgets are **review targets**, not gates: exceeding one prompts a look, not a failed build.

---

## 10. Tooling

- **Criterion** — timed benches; statistical handling of variance suits the noisy local environment.
- **dhat** — Rust-native heap profiling; portable allocation bytes/counts; the backbone of continuous memory tracking. (heaptrack/massif remain options for deep local investigation, but dhat is the integrated, portable default.)
- **Peak RSS** — OS-level measurement (`/proc` on Linux, platform API on macOS), device-bound.
- **GPU frame-time** — wgpu timestamp queries / frame pacing on real hardware; **not** agent-runnable.
- **`vello_cpu` render cost** — Criterion-timed, portable render-complexity proxy (from Spec 02's rasterizer).

---

## 11. Local-Only Tracking Discipline

**Decision (D3): benchmarks are tracked, not gated.** This is a deliberate departure from Spec 01's hard CI gates, forced by hardware variance:

- Runs are **local**; results diff against the committed baseline.
- Regressions are **surfaced for human review**, not enforced — no benchmark ever fails CI.
- The **baseline is a committed artifact** updated intentionally, so history shows how performance and memory move over releases. This *is* the "continuous" in continuous tracking: a portable signal (§5) tracked over time in version control.
- The discipline is: run locally before/after significant changes, diff, review deltas, update the baseline with intent when a shift is justified and explained.

This gives regression *visibility* without the false-failure noise a hardware-sensitive gate would produce.

---

## 12. The `vello_cpu` / GPU Parity Cadence

Spec 02 introduced a second render path (`vello_cpu`) alongside the production GPU path, and flagged that the two pinned Vello crates can drift as they version forward. The CPU/GPU parity check (same scene both ways, expected to agree within tolerance) is what catches that drift — and it needs a **cadence**, which this spec owns:

- Run the parity check **on every Vello version bump** and on a regular local cadence, on Kevin's GPU hardware (it needs a GPU).
- A parity divergence is a signal that the crates have drifted or a render change affects the paths differently — investigated, not ignored.
- This keeps Spec 02's conformance goldens trustworthy (they're rendered on `vello_cpu`, so `vello_cpu` must stay faithful to what users see on GPU).

---

## 13. Key Decisions (ADR-style)

**D1 — Allocation metrics are the portable memory signal; RSS is a device-local check.** Bytes/counts barely vary by hardware, so they're what's continuously tracked; RSS is a reality check on real devices. Tradeoff: allocation metrics don't capture fragmentation/RSS overhead — covered by the device-local RSS check.

**D2 — Budgets calibrated against the 8 GB floor, not guessed.** Measured targets with headroom to coexist on the floor device; committed calibration record. Tradeoff: a calibration step before budgets mean anything — worth it to avoid arbitrary numbers.

**D3 — Tracked, not gated.** Hardware variance makes cross-machine pass/fail meaningless; a committed baseline diffed locally gives visibility without false failures. Tradeoff: no automatic enforcement — mitigated by the review discipline and the committed baseline's visible history.

**D4 — Scale corpus separate from the conformance corpus.** Benchmarking stresses size; conformance stresses breadth; the pathological tier is deliberately abusive. Tradeoff: another corpus to maintain — necessary, since feature fixtures don't exercise scale.

**D5 — `vello_cpu` doubles as a portable render proxy; this spec owns the parity cadence.** Reuses Spec 02's rasterizer for a hardware-independent render signal and keeps the two render paths honest. Tradeoff: the proxy isn't the production path — that's why device-bound GPU frame-time is still measured.

---

## 14. Milestones & Acceptance Criteria

**M1 — Harness + two-axis split.** Benchmark harness at the monorepo root; portable vs. device-bound axes cleanly separated; Criterion + dhat wired. *Accept:* the portable set runs headless in the agent environment with no GPU; the device set is clearly marked as hardware-only.

**M2 — Portable benches.** Model/layout/IO/export/style-resolution op-cost and allocation benches; `vello_cpu` render-cost proxy. *Accept:* each named §6 target has a portable bench producing stable allocation metrics; style resolution is stressed on deep chains × many styles and its scaling is visible.

**M3 — Continuous memory tracking.** Committed portable-metric baseline per corpus tier; diff-and-review flow; the Arc steady-state guard. *Accept:* a deliberate regression (e.g. cloning a `Renderer` instead of sharing the `Arc`) shows up as a baseline delta; the guard flags it.

**M4 — Leak detection.** Long-session open/edit/close returns to near-baseline (Arc cycles); caches proven bounded under the pathological tier; Loro history-growth bench. *Accept:* a seeded leak (a retained document, an unbounded cache) is caught; Loro history behavior over a long session is measured and reported.

**M5 — Device benches + budgets.** GPU frame-time and peak-RSS on Kevin's hardware; per-tier RSS budgets calibrated against the 8 GB floor; calibration record committed. *Accept:* frame-time and peak RSS are measurable on real hardware; budgets trace to the calibration data, not a guess.

**M6 — Discipline + parity cadence.** The tracked-not-gated discipline (§11) documented; the CPU/GPU parity cadence (§12) established. *Accept:* the baseline-update discipline is written down; the parity check has a defined trigger (Vello bump + regular cadence) and a divergence is actionable.

---

## 15. Out of Scope

- Any CI gate or automatic build failure from benchmarks (§11).
- Cross-machine result comparison.
- The optimizations themselves (measurement informs them; they're separate work).
- Adding new caches or changing the render architecture (measured here, changed elsewhere).
- Presentation/Spreadsheet benchmark suites (the harness is shared; their corpora/suites are their own work, as with Spec 02).
