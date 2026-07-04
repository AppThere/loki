<!--
SPDX-License-Identifier: Apache-2.0
-->

# loki-bench — benchmarking & continuous memory tracking (Spec 06)

Shared harness at the monorepo root for Loki's performance and **continuous
memory** benchmarks. Implements the [Spec 06](../docs/adr/spec-06-benchmarking-and-memory-tracking.md)
**two-axis split**; see the [audit](../docs/adr/spec-06-benchmarking-audit.md) for
ground truth.

## The two axes (§5)

| Axis | Metrics | Runs where |
|------|---------|------------|
| **Portable** | heap allocation bytes/counts (dhat), op counts, `vello_cpu` render cost | agent / CI / any dev machine — **headless, no GPU** |
| **Device-bound** | GPU frame-time, wall-clock latency, real peak RSS | real hardware only |

`Axis` / `Metric` (in `src/axis.rs`) encode this in code: `Metric::axis()` maps
allocation bytes/counts to `Portable` (the continuously-tracked signal, decision
D1) and RSS/frame-time to `DeviceBound`.

## What M1 provides

- The `Axis` / `Metric` model and its mapping (tested, headless).
- `measure(|| workload) -> AllocStats` — portable allocation bytes/counts via dhat.
- `dhat_global_allocator!()` — opt a bench binary into dhat recording.
- Criterion wired via `benches/`.

```bash
cargo test  -p loki-bench                              # axis + measurement unit tests (headless)
cargo bench -p loki-bench --bench portable_smoke       # Criterion runs headless
cargo bench -p loki-bench --bench portable_alloc       # dhat captures allocation bytes/counts headless
cargo bench -p loki-bench --bench device_frame_time    # hardware-only: prints a skip notice headless
```

## Continuous memory tracking (M3)

`baselines/portable.txt` is the committed per-tier baseline (one line per key).
Each run diffs against it and surfaces deltas for **review** — never a CI failure
(§11). The Arc steady-state guard asserts sharing allocates nothing.

```bash
cargo bench -p loki-bench --bench baseline               # diff current run vs committed baseline
cargo bench -p loki-bench --bench baseline -- --update   # rewrite the baseline (intentional)
cargo bench -p loki-bench --bench arc_steady_state       # assert Arc::clone allocates 0
```

## Leak detection (M4)

Residual live-heap measurement (`residual_after`) + a pure `classify_leak`
verdict catch the §7 culprits: Arc cycles / retained documents, unbounded
caches, and Loro history growth.

```bash
cargo bench -p loki-bench --bench leak_detection     # clean cycle Bounded; seeded leaks caught
cargo bench -p loki-bench --bench leak_loro_history  # reports oplog growth over a long session
```

## Device benches + budgets (M5)

Peak RSS is measurable now (Linux `/proc`; device-local numbers). Budgets are
committed (`baselines/rss_budgets.txt`) and reviewed, not gated (§11). GPU
frame-time is device-only (needs a GPU).

```bash
cargo bench -p loki-bench --bench device_rss               # peak RSS per tier + budget review
cargo bench -p loki-bench --bench device_rss -- --update   # recalibrate budgets (1.5× measured peak)
cargo bench -p loki-bench --bench device_frame_time --features device   # GPU frame-time (hardware only)
```

The committed budgets are **agent-provisional** — they under-count real devices
(no GPU textures); see [`docs/adr/spec-06-calibration.md`](../docs/adr/spec-06-calibration.md)
for the recalibration steps on Kevin's hardware (audit BM-14).

## Not yet (later milestones)

The CPU/GPU parity cadence (M6), blocked on Spec 02 landing the `vello_cpu` CPU
render path (audit finding BM-3).
