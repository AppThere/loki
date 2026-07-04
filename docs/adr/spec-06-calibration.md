<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 06 — Peak-RSS Budget Calibration Record

| | |
|---|---|
| **Status** | **Provisional** — measured in the headless agent; **awaits on-device recalibration** (BM-14). |
| **Spec** | [spec-06-benchmarking-and-memory-tracking.md](spec-06-benchmarking-and-memory-tracking.md) §9 (Budgets & Calibration, decision D2) |
| **Companion** | [spec-06-benchmarking-audit.md](spec-06-benchmarking-audit.md) (finding BM-14) |
| **Artifact** | [`loki-bench/baselines/rss_budgets.txt`](../../loki-bench/baselines/rss_budgets.txt) (the committed per-tier budgets) |

This is the committed calibration record §9 requires: the measured distributions,
the chosen budgets, and the device / date / tool versions they trace to — so
budgets are **traceable to data, not guessed** (D2). Budgets are **review
targets, never CI gates** (§11).

## Method (D2)

`cargo bench -p loki-bench --bench device_rss` builds and lays out each corpus
tier (held live) and reads the OS peak RSS (`/proc/self/status` `VmHWM`).
`-- --update` writes each tier's budget as **1.5 × measured peak** (50% headroom
over current behaviour). The 8 GB floor (MacBook Neo, A16, 2026) is the ceiling
the budgets must respect: Loki has to coexist with an OS and a browser, so the
target is **sub-gigabyte per realistic tier** — which the ~750 MB steady-state
reference (the shared-`Arc` render-cache win) shows is achievable.

## Measurement device (this record)

| | |
|---|---|
| Environment | **Headless CI agent — Linux, no GPU** |
| `rustc` | 1.94.1 |
| `dhat` | 0.3.3 |
| Date | 2026-07-04 |
| Corpus | scale tiers: small 10 paras, medium 60, large 250 (`benches/support`) |

## Measured peak RSS + chosen budgets

Process baseline peak RSS: **2.4 MiB**.

| Tier | Measured peak RSS | Budget (1.5×) |
|------|-------------------|---------------|
| small (10 paras) | 9.3 MiB | 13.9 MiB |
| medium (60 paras) | 12.1 MiB | 18.2 MiB |
| large (250 paras) | 22.9 MiB | 34.4 MiB |

## ⚠️ Why these are provisional (BM-14)

**These numbers are agent measurements and materially under-count real devices.**
The agent has **no GPU**, so none of the resident cost that actually dominates on
device is present:

- **No GPU page textures** — the tiered render cache (Hot/Warm/Cold, the
  ~240 MB→45 MB and >3 GB idle findings) is entirely device-side.
- **Different allocator / OS accounting** than Windows or macOS.
- **No wgpu / driver working set.**

So the real device RSS is expected in the hundreds of MB (the ~750 MB
steady-state class), not tens of MB. **Before these budgets are authoritative,
Kevin must:**

1. Run `cargo bench -p loki-bench --bench device_rss -- --update` on the
   **Windows + RTX 3050** box and on the **MacBook A16** (add the macOS `getrusage`
   / Windows `GetProcessMemoryInfo` reader — Spec 06 M5 §10; the Linux `/proc`
   reader is done and verified).
2. Commit the device budgets, updating this record with the device rows, and set
   headroom so the large tier still leaves the 8 GB floor room for an OS + browser.

Until then the committed budgets exercise the **mechanism** (measure → calibrate →
review) with real-but-agent-local numbers; they are not device budgets.

## GPU frame-time (the other M5 metric)

GPU frame-time is device-only (wgpu timestamp queries on the production paint
path) and **cannot run in the agent** — it is the `device_frame_time` target,
gated behind the `device` feature, and is executed and recorded on Kevin's GPU
hardware as part of the same on-device pass.
