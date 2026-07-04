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

## Not yet (later milestones)

Per-target portable benches (M2), the committed baseline + diff (M3), leak
detection (M4), device benches + budgets (M5), and the CPU/GPU parity cadence
(M6). The `device` feature currently only **marks** the device axis — the GPU/RSS
measurement itself is M5. The `vello_cpu` render-cost proxy is blocked on Spec 02
landing the CPU render path (audit finding BM-3).
