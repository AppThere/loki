<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 06 — Tracking Discipline & Parity Cadence

| | |
|---|---|
| **Status** | Active — the standing process for Spec 06's benchmarks. |
| **Spec** | [spec-06-benchmarking-and-memory-tracking.md](spec-06-benchmarking-and-memory-tracking.md) §11 (discipline, D3) + §12 (parity cadence, D5) |
| **Tools** | `loki-bench` — `baseline`, `device_rss`, `leak_*`, `arc_steady_state`, `parity_status` |

This is the written-down discipline (Spec 06 M6): how the benchmarks are run,
reviewed, and committed, and when the CPU/GPU parity check must run. Two rules
frame everything: **benchmarks are tracked, never gated** (§11), and the
**parity check runs on every Vello bump** (§12).

---

## 1. Tracked, not gated (§11 / D3)

Hardware variance makes a cross-machine pass/fail meaningless, so **no benchmark
ever fails CI**. Instead, a committed baseline is diffed locally and regressions
are surfaced for human review. The committed artifacts *are* the continuous
record — their git history shows how memory and cost move over releases:

| Artifact | What it holds | Tool |
|---|---|---|
| [`loki-bench/baselines/portable.txt`](../../loki-bench/baselines/portable.txt) | portable allocation metrics per §6 target × tier + the Arc-guard metric | `baseline` |
| [`loki-bench/baselines/rss_budgets.txt`](../../loki-bench/baselines/rss_budgets.txt) | per-tier peak-RSS budgets (review targets) | `device_rss` |
| [`loki-bench/baselines/parity_marker.txt`](../../loki-bench/baselines/parity_marker.txt) | Vello version last confirmed by a parity run | `parity_status` |
| [`spec-06-calibration.md`](spec-06-calibration.md) | the RSS budget calibration record | — |

### The workflow

Run **before and after any significant change** (anything touching the model,
layout, rendering, IO/export, style resolution, the shared-`Arc` caches, or a
dependency bump — i.e. anything a §6 target exercises):

```bash
cargo bench -p loki-bench --bench baseline           # portable allocation diff vs committed baseline
cargo bench -p loki-bench --bench arc_steady_state   # Arc::clone must allocate 0
cargo bench -p loki-bench --bench device_rss         # peak RSS per tier vs budgets (device-local)
cargo bench -p loki-bench --bench leak_detection     # open/edit/close leak-free; seeded leaks caught
cargo bench -p loki-bench --bench leak_loro_history  # long-session oplog growth report
```

Then:

1. **Review the deltas.** `baseline` prints `ok` / `improved` / `REGRESSED` per
   key; `device_rss` prints headroom vs budget. A `REGRESSED` line or an `OVER`
   budget is a prompt to look, not a failure.
2. **Explain any real change.** A justified shift (a feature that legitimately
   costs more, an intentional optimisation) is fine; an *unexplained* regression
   is the thing to catch.
3. **Update the baseline intentionally.** When a shift is understood and
   accepted, regenerate and commit the artifact so it becomes the new reference:
   ```bash
   cargo bench -p loki-bench --bench baseline   -- --update
   cargo bench -p loki-bench --bench device_rss -- --update   # on the measuring device
   ```
   Never update silently to make a diff "go away" — the commit that moves a
   baseline should say *why*.

That intentional, explained, committed update is the "continuous" in continuous
tracking: the reference moves in version control, with a reason attached.

---

## 2. CPU/GPU parity cadence (§12 / D5)

Spec 02 renders conformance goldens on `vello_cpu` while the app paints on GPU
`vello`. The two pinned crates (`vello` 0.6 + `vello_cpu`) can drift as they
version forward; the **parity check** — the same scene rendered both ways,
expected to agree within tolerance — is what catches that drift and keeps the
goldens trustworthy (they are only meaningful if `vello_cpu` stays faithful to
what users see on GPU).

### Triggers

- **On every Vello version bump — mechanical.** `parity_status` reads the pinned
  `vello` version from `Cargo.lock` and compares it to the last confirmed version
  in `parity_marker.txt`; a bump prints **`[DUE]`**. Run it after any dependency
  update:
  ```bash
  cargo bench -p loki-bench --bench parity_status
  ```
- **On a regular local cadence — calendar.** Run the parity check at least once
  per release even without a bump, to catch drift a render change (not a version
  change) could introduce.

### Where it runs

On **GPU hardware** (Kevin's Windows + RTX 3050 / MacBook A16) — the check needs a
real GPU. The `parity_status` *trigger* is headless (it only reads versions), but
the *check* it prompts is device-bound.

### Responding to a divergence

A parity divergence means the crates have drifted or a render change hits the two
paths differently. It is **actionable, not ignorable**:

1. Investigate the cause (crate drift vs. a scene/render change affecting CPU and
   GPU differently).
2. **Do not update `parity_marker.txt` until parity is restored.** The marker
   records a *confirmed* run; recording an unconfirmed version would silently bless
   drifted goldens.
3. Once the check passes on device, record it:
   ```bash
   cargo bench -p loki-bench --bench parity_status -- --update   # note date/device/tolerance in the marker
   ```

### Current status (honest)

The parity **check** cannot run yet: it needs a GPU *and* Spec 02's `vello_cpu`
render path, which is still pending implementation (audit **BM-3**). So
`parity_marker.txt` is intentionally **unconfirmed**, and `parity_status`
correctly reports `[DUE]` for the current pin. The trigger and process are defined
and in place now; the check executes once Spec 02 lands the CPU render path.
