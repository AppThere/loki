<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spec 08 Phase 2 — texture residency baseline

| Field | Value |
| --- | --- |
| Task | T2.5 + T2.5a (instrument and baseline) |
| Date | 2026-07-26 |
| Instrument | `loki-bench/benches/texture_residency.rs` (permanent, headless) |
| Counter | `appthere_canvas::residency::TextureResidency` |
| Model | `appthere_canvas::residency` — the same `visible_window` production mounts with |
| Machine | headless x86-64 dev container; **no GPU involved** — see §5 |

Taken **before** Phase 2 changes anything. Spec 09's E0 and S9-1 worked because
the instrument and the baseline came first; a budget imposed before a baseline
exists has nothing to be compared against.

---

## 1. What was measured

A scroll traversal, not a single viewport. The viewport steps through the whole
document in half-screen increments; at each stop the mounted set is recomputed
with the production rule and the counter records the arriving and departing
tiles. The figure reported is the **peak across the traversal**, because a
budget has to hold at every scroll offset rather than at the one an author
picked.

Fixed conditions for every row below: **viewport 900 CSS px tall, 24 px page
gap** (`appthere_ui::tokens::layout::PAGE_GAP_PX`), tiles at full resolution.
Those are S0.2 §3's assumptions, chosen so the baseline is directly comparable
with the spike it is checking.

---

## 2. Baseline — zoom × device scale factor

US Letter, 500 pages. `tiles` is the mounted count at the peak.

| zoom | dsf | tiles | bytes | MiB |
| ---: | ---: | ---: | ---: | ---: |
| 25% | 1.0 | 11 | 2,369,664 | 2.3 |
| 25% | 2.0 | 11 | 9,478,656 | 9.0 |
| 25% | 3.0 | 11 | 21,326,976 | 20.3 |
| 50% | 1.0 | 6 | 5,170,176 | 4.9 |
| 50% | 2.0 | 6 | 20,680,704 | 19.7 |
| 50% | 3.0 | 6 | 46,531,584 | 44.4 |
| 100% | 1.0 | 4 | 13,787,136 | 13.1 |
| 100% | 2.0 | 4 | 55,148,544 | 52.6 |
| 100% | 3.0 | 4 | 124,084,224 | 118.3 |
| **200%** | **2.0** | **3** | **165,445,632** | **157.8** |
| 200% | 1.0 | 3 | 41,361,408 | 39.4 |
| 200% | 3.0 | 3 | 372,252,672 | 355.0 |
| 400% | 1.0 | 2 | 110,297,088 | 105.2 |
| 400% | 2.0 | 2 | 441,188,352 | 420.8 |
| 400% | 3.0 | 2 | 992,673,792 | 946.7 |

ISO A4 — the default page of a new document — for reference:

| zoom | dsf | tiles | bytes | MiB |
| ---: | ---: | ---: | ---: | ---: |
| 100% | 1.0 | 4 | 14,266,592 | 13.6 |
| 100% | 2.0 | 4 | 57,005,040 | 54.4 |
| 200% | 2.0 | 3 | 171,069,000 | 163.1 |
| 200% | 3.0 | 3 | 384,864,840 | 367.0 |

### 2.1 Document length

| pages | zoom | dsf | bytes |
| ---: | ---: | ---: | ---: |
| 10 | 100% | 1.0 | 13,787,136 |
| 100 | 100% | 1.0 | 13,787,136 |
| 500 | 100% | 1.0 | 13,787,136 |
| 10 | 200% | 2.0 | 165,445,632 |
| 100 | 200% | 2.0 | 165,445,632 |
| 500 | 200% | 2.0 | 165,445,632 |

Identical, not merely close. Asserted in the bench.

---

## 3. Findings, per L9-019

### Observed

- **Textures are already windowed.** A 500-page document never mounts more than
  11 tiles at any zoom swept, and resident texture bytes are **byte-identical**
  across 10, 100 and 500 pages. Conditions: 900 px viewport, 24 px gap, the
  `visible_window` rule as it stands (visible range grown one screen each side).
- **The unbounded axis is zoom × device scale factor.** 2.3 MiB at 25%/1× to
  946.7 MiB at 400%/3× — a 419× span from two user-reachable controls, on the
  same document, on the same machine.
- **§3.2's headline reproduces exactly.** 200% on a HiDPI display reads
  **165,445,632 bytes = 165.4 MB decimal**, against S0.2 §3's predicted 165.4 MB
  (3 tiles × 55.15 MB). The tile counts match the spike's table row for row
  (11 / 6 / 4 / 3 at 25 / 50 / 100 / 200%). Two independently derived models
  agreeing to the byte is stronger than either alone.
- **Residency is exactly quadratic in device scale factor and sub-quadratic in
  zoom.** The dsf multiplies tile size and nothing else — the mounting window is
  computed in CSS px — so doubling it is exactly 4×, asserted in the bench.
  Zoom multiplies tile size *and* shrinks the mounted count (taller pages, same
  window), so 100% → 200% costs 3.0×, not 4×. **Total residency is therefore
  roughly linear in zoom and quadratic in dsf**, which is not what "zoom × DPI"
  suggests at a glance and is the fact a budget should be derived against.
- **The counter balances.** Every subject ends with `resident == 0` and
  `allocs == frees`; the ordering control re-read the first subject identically
  after 40-odd other subjects had run.

### Not established

- **That Blitz's physical tile size is exactly `round(css × dsf)`.** The model
  assumes it. A ±1 px disagreement per axis is under 0.1% of a page texture, but
  it is an assumption about the tree, not an observation of it.
- **Driver-side overhead.** Row-pitch alignment, heap granularity and any
  implicit staging sit on top of every figure here and are invisible without a
  device. This is the one thing Phase 2's closing gate confirms (R16, revised).
- **The transient peak within a frame.** The traversal releases departed tiles
  before allocating arriving ones. Blitz's `release` and `render` are separate
  callbacks and their relative order within a frame was not determined, so a
  pessimistic frame could briefly hold one extra tile — up to +33% at 200%,
  where only three are resident.
- **Anything about a real GPU's memory pressure.** These are *requested* bytes
  for page textures only. Vello's own scratch buffers, the swapchain, and
  whatever else the process holds are outside the count.

### What would settle it

The same counter, read on device. It is process-wide and already fed by the
real allocation and release sites, so an on-device run needs no second
instrument — only a way to print `TextureResidency::snapshot()`. That single
run answers all three of the first "not established" items at once.

---

## 4. Prediction for T2.1–T2.3, recorded before implementation (L9-013)

**Governing metric: peak resident texture bytes across zoom × DPI**, as
measured above. Not tile count, not RSS, not frame time.

**Predicted effect of the budget.**

1. **Every row at or below 100%/2× is unchanged.** The proposed 256 MiB ceiling
   and a `DeviceProfile`-derived baseline near 64 MiB both sit above 52.6 MiB,
   so a budget that fires there would be mis-derived. Rows ≤ 52.6 MiB must read
   **byte-identical** afterwards.
2. **200%/3× (355.0 MiB) and both 400% dsf ≥ 2 rows (420.8, 946.7 MiB) come
   down to the budget.** These are the rows Phase 2 exists for.
3. **200%/2× — the headline 157.8 MiB — is the interesting one.** It is *under*
   a 256 MiB ceiling and *over* a 64 MiB baseline, so what happens to it is
   decided entirely by the derivation, and it is the single row that reveals
   whether the `DeviceProfile` inputs are being used or ignored. If it is
   unchanged on a 16 GB machine and reduced on a 4 GB one, from the same binary,
   L08-011 is true in practice and not merely asserted.
4. **The saving must land in rasterisation scale, not in eviction.** T2.2 is
   explicit: reduce scale for off-centre pages before dropping anything visible.
   So the *tile count* at the peak must be unchanged by the budget at every row,
   while the bytes per off-centre tile fall. A saving that showed up as fewer
   tiles would mean the budget is evicting the visible set — the total would
   improve and the model would be wrong, which is the L9-013 failure mode
   exactly.

**Falsification.** Any of: a row ≤ 52.6 MiB moving; the peak tile count falling;
the same document reading the same budget on devices with materially different
RAM. Any one of those means the implementation does not match this model,
whatever the totals say.

---

## 5. Why this ran without hardware

Every term is CPU-side. Which pages mount is viewport arithmetic; a page texture
is `width × height × 4` for a known format (`Rgba8Unorm`, fixed at the
allocation site). wgpu is called *after* all of that is decided.

R16 was revised on this reasoning after Spec 09 found layout measurable
headlessly for the same reason. L08-021 is the general form: name the component
that needs hardware before recording anything as blocked on it. Phase 0's
texture figures were code-derived because "no GPU was available" — but the
figures never needed one.

---

## 6. Re-measurement after T2.1–T2.3

See §7 below, added when the budget lands.
