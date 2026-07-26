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

Same bench, same conditions, with the mounted set and each tile's rasterisation
scale coming from `plan_residency` — the function the renderer itself calls.
Three device classes, **one binary**: the budget is derived from measured
available RAM, so what differs between these tables is the machine and nothing
else (L08-011, D-08).

`!` marks a row where the plan reported it cannot reach the budget without
dropping a visible page or rendering below legibility.

### Phone — 4 GiB, ~2 GiB available → **32 MiB budget**

| zoom | dsf | before MiB | after MiB | tiles | change |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 25% | 1.0 | 2.3 | 2.3 | 11 | unchanged |
| 25% | 3.0 | 20.3 | 20.3 | 11 | unchanged |
| 50% | 3.0 | 44.4 | 31.4 | 6 | −29% |
| 100% | 1.0 | 13.1 | 13.1 | 4 | unchanged |
| 100% | 2.0 | 52.6 | 29.6 | 3 | −44% |
| 200% | 2.0 | 157.8 | 32.0 | 2 | −80% |
| 200% | 3.0 | 355.0 | 32.0 | 2 | −91% |
| 400% | 2.0 | 420.8 | 32.0 | 2 | −92% |
| 400% | 3.0 | 946.7 | 59.2 | 2 | −94% ! |

### Design floor — 8 GiB, ~4 GiB available → **64 MiB budget**

| zoom | dsf | before MiB | after MiB | tiles | change |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 50% | 3.0 | 44.4 | 44.4 | 6 | unchanged |
| 100% | 2.0 | 52.6 | 52.6 | 4 | unchanged |
| 100% | 3.0 | 118.3 | 62.9 | 4 | −47% |
| 200% | 1.0 | 39.4 | 39.4 | 3 | unchanged |
| **200%** | **2.0** | **157.8** | **61.5** | **2** | **−61%** |
| 200% | 3.0 | 355.0 | 61.5 | 1 | −83% |
| 400% | 3.0 | 946.7 | 61.5 | 1 | −94% |

### Desktop — 16 GiB, ~11 GiB available → **176 MiB budget**

| zoom | dsf | before MiB | after MiB | tiles | change |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100% | 3.0 | 118.3 | 118.3 | 4 | unchanged |
| **200%** | **2.0** | **157.8** | **157.8** | **3** | **unchanged** |
| 200% | 3.0 | 355.0 | 169.1 | 2 | −52% |
| 400% | 1.0 | 105.2 | 105.2 | 2 | unchanged |
| 400% | 2.0 | 420.8 | 169.1 | 2 | −60% |
| 400% | 3.0 | 946.7 | 169.1 | 2 | −82% |

## 7. Did it land where predicted? (L9-013)

**Prediction 1 — rows already under budget are byte-identical.** ✅ Held, and
asserted in the bench rather than eyeballed. Every row at or below the device's
budget reads the same byte value before and after. One refinement the prediction
did not anticipate: it said "every row at or below 100%/2×", which is only true
of a device whose budget is at least 52.6 MiB. On the phone (32 MiB) that row is
legitimately over budget and moves. The prediction silently assumed the design
floor; the property it was reaching for — *unpressured rows are untouched* — is
what actually holds.

**Prediction 2 — the high-zoom rows come down to the budget.** ✅ Held.
200%/3× and both 400% rows at dsf ≥ 2 are reduced on every device class.

**Prediction 3 — the discriminating row.** ✅ Held **exactly**. 200%/2× —
157.8 MiB, above a 64 MiB baseline and below a 256 MiB ceiling — is
**unchanged on the 16 GiB desktop and 61.5 MiB on the 8 GiB machine, from the
same binary**. That is the row whose behaviour is decided entirely by the
derivation rather than by a clamp, and it is the strongest evidence available
that L08-011 is true in practice and not merely asserted (R24).

**Prediction 4 — the saving lands in rasterisation scale, not eviction.**
❌ **Mis-stated, and the falsification criterion as written would have fired.**
The prediction said "the peak tile count must be unchanged at every row"; it is
not — 3 tiles become 2, and at 200%/3× on the 8 GiB machine 1. The error is in
the prediction, not the implementation: L08-002 forbids evicting **visible**
content, and the tiles being dropped are the off-centre pre-render margin, which
step 3 of the policy drops *after* exhausting the scale ladder and which was
already blank for any page outside the window. "Tile count unchanged" was a
proxy for "nothing visible is evicted", and it is a bad proxy.

The property actually worth asserting is the direct one, and the bench now
checks it **at every scroll offset of the traversal** rather than at a single
position: every strictly-visible page is present in the plan. That assertion is
what would fire if the budget ever started paying for itself with the visible
set — and it is stricter than the tile-count proxy, not weaker.

Recording this the way L9-013 asks: the *model* was right (scale first, then the
margin, never the visible set) and the *metric chosen to falsify it* was wrong.
A prediction that is confirmed by a bad proxy is worth less than one refuted by
a good one.

### Both conditions (L9-014)

The targeted condition is high-zoom residency, and it moved the right way. The
untargeted condition — **rows already inside the budget** — is reported beside
it in every table above and did not move at all, byte for byte. That is the
check Spec 09's S9-1 needed and did not have: it moved its target 35 B/char the
right way and read-only 11 B/char the wrong way, and only a two-condition report
caught it.

### What remains not established

- **The device scale factor is still 1.0 in the running application.**
  `loki_text::texture_budget::device_scale_factor` returns a constant with a
  `TODO(device-profile-dpr)`: Blitz owns the real value and hands it to the
  paint source as `render`'s `scale`, but nothing surfaces it to the layer that
  must decide what to mount. Every table above sweeps DPI correctly; the shipped
  binary currently plans as though every display were standard-DPI, which makes
  the budget bind later than it should rather than earlier. The failure mode is
  a missed saving, never a blank page — but the axis Phase 2 exists to bound is
  zoom × **DPI**, and half of it is not yet wired.
- **Whether a reduced-scale tile looks acceptable.** The policy, the arithmetic
  and the invalidation are unit-tested; how 0.25 scale reads at 400% zoom is a
  question for a screen. R5.
- **The `anyrender_vello` brush-scaling patch has never rendered a frame.** It
  is inert unless a source returns a mismatched texture size, so it cannot
  regress anything today, but the first time the budget binds on a real display
  is the first time it runs.
- **Driver-side overhead**, unchanged from §3.

---

## 8. Re-measurement after the r15 policy change (ADR L08-026)

### What changed and why

The r14 planner had four steps, the last of which reduced the **visible** set's
rasterisation scale once it exceeded the byte budget. That was presented as a
400%-on-a-3x-display corner. It is not one.

A probe over the trigger conditions (temporary, run 2026-07-26) found that once
the real device scale factor is wired (R27), step 4 fires during ordinary
reading:

| device | 200% / 2x | 200% / 3x | 400% / 3x |
| --- | --- | --- | --- |
| phone 4 GiB | **100%** of scroll offsets, scale 0.55 | 100%, 0.37 | 100%, 0.25 |
| design floor 8 GiB | **40%** of offsets, scale 0.76 | **100%**, 0.51 | 100%, 0.25 |
| desktop 16 GiB | never | **40%**, 0.85 | 100%, 0.42 |

The 40% figure is the important one, and it is not "40% of the time" in a way
that averages out: step 4 fired at exactly those offsets where a **page boundary
sits inside the viewport**, because that is when two full-page textures are
resident at once. So body text did not sit uniformly soft — it softened and
re-sharpened as the reader scrolled across each boundary. Intermittent is worse
than constant here.

That is a rendering defect wearing a memory policy's clothes. The byte target is
our invention; the reader's perception is not. So:

- **The target may only buy memory back from work the user cannot see.** Steps
  1–3 (full scale, ladder off-centre tiles, drop off-centre tiles) are unchanged.
- **Step 4 now stops.** If the visible set alone exceeds the target, it is mounted
  at full scale and `over_budget` is reported. Being over target is an ordinary,
  reported outcome rather than a failure to correct.
- **A new step 5 degrades visible scale only above a survival ceiling**, far
  above the target, where the alternative is not "slightly more memory" but the
  OS killing the process.

"Decline to mount instead of softening" was considered and rejected for *visible*
content: refusing to mount is a blank page where the user is reading, which is
strictly worse than a soft one and contradicts the never-drop-visible criterion.
Declining is the right answer for off-centre tiles, and step 3 already does it.

### Calibrating the ceiling

Both candidate divisors were measured rather than argued.

`available / 4` never softens an ordinary operating point on any device, but it
permits a **946.7 MiB** peak on the 8 GB design floor, and a gigabyte of texture
in a document viewer is not defensible beside Spec 09's layout residency on the
same machine.

`available / 8` caps that at 512 MiB and still clears every ordinary point with
headroom — the worst ordinary case is 236.7 MiB, at 200% on a 3x display. It does
bind earlier on a genuinely memory-poor device (a phone under ~1.9 GiB available
softens at 200%/3x), and **that is the regime working, not a regression**: a
device that cannot afford two full-resolution pages does not get sharp text by
refusing to degrade, it gets an OOM kill.

`available / 8` was chosen.

### Measured, after the change

```
Under budget — phone 4 GiB (32 MiB budget, AvailableRam)
    zoom    dsf  before MiB   after MiB    tiles      change
  (`*` = over target, visible pages still full scale — the L08-026 outcome; `!` = survival ceiling bound and visible scale reduced)
     25%    1.0         2.3         2.3       11   unchanged
     25%    2.0         9.0         9.0       11   unchanged
     25%    3.0        20.3        20.3       11   unchanged
     50%    1.0         4.9         4.9        6   unchanged
     50%    2.0        19.7        19.7        6   unchanged
     50%    3.0        44.4        31.4        6        -29%
    100%    1.0        13.1        13.1        4   unchanged
    100%    2.0        52.6        29.6        3        -44%
    100%    3.0       118.3        59.2        2      -50% *
    200%    1.0        39.4        27.9        3        -29%
    200%    2.0       157.8       105.2        2      -33% *
    200%    3.0       355.0       236.7        2      -33% *
    400%    1.0       105.2       105.2        2  unchanged *
    400%    2.0       420.8       245.9        2      -42% !
    400%    3.0       946.7       256.0        1      -73% !

Under budget — design floor 8 GiB (64 MiB budget, AvailableRam)
    zoom    dsf  before MiB   after MiB    tiles      change
  (`*` = over target, visible pages still full scale — the L08-026 outcome; `!` = survival ceiling bound and visible scale reduced)
     25%    1.0         2.3         2.3       11   unchanged
     25%    2.0         9.0         9.0       11   unchanged
     25%    3.0        20.3        20.3       11   unchanged
     50%    1.0         4.9         4.9        6   unchanged
     50%    2.0        19.7        19.7        6   unchanged
     50%    3.0        44.4        44.4        6   unchanged
    100%    1.0        13.1        13.1        4   unchanged
    100%    2.0        52.6        52.6        4   unchanged
    100%    3.0       118.3        62.9        4        -47%
    200%    1.0        39.4        39.4        3   unchanged
    200%    2.0       157.8       105.2        2      -33% *
    200%    3.0       355.0       236.7        2      -33% *
    400%    1.0       105.2       105.2        2  unchanged *
    400%    2.0       420.8       420.8        2  unchanged *
    400%    3.0       946.7       512.0        2      -46% !

Under budget — desktop 16 GiB (176 MiB budget, AvailableRam)
    zoom    dsf  before MiB   after MiB    tiles      change
  (`*` = over target, visible pages still full scale — the L08-026 outcome; `!` = survival ceiling bound and visible scale reduced)
     25%    1.0         2.3         2.3       11   unchanged
     25%    2.0         9.0         9.0       11   unchanged
     25%    3.0        20.3        20.3       11   unchanged
     50%    1.0         4.9         4.9        6   unchanged
     50%    2.0        19.7        19.7        6   unchanged
     50%    3.0        44.4        44.4        6   unchanged
    100%    1.0        13.1        13.1        4   unchanged
    100%    2.0        52.6        52.6        4   unchanged
    100%    3.0       118.3       118.3        4   unchanged
    200%    1.0        39.4        39.4        3   unchanged
    200%    2.0       157.8       157.8        3   unchanged
    200%    3.0       355.0       236.7        2      -33% *
    400%    1.0       105.2       105.2        2   unchanged
    400%    2.0       420.8       420.8        2  unchanged *
    400%    3.0       946.7       946.7        2  unchanged *

Ordering control: OK — first subject re-read identically last (165445632 bytes, 3 tiles, 500 allocations).
```

`*` marks a row that exceeds the byte target with **visible pages still at full
scale** — the L08-026 outcome, and now the common shape of budget pressure at
high DPI. `!` marks the survival ceiling binding, the only rows where a page the
user is looking at was reduced.

### What this cost, stated plainly

Peak residency went **up** at several operating points relative to r14, and that
is the trade being made on purpose:

| | r14 (soften visible) | r15 (protect visible) | cost |
| --- | ---: | ---: | ---: |
| 8 GiB, 200% / 2x | 61.5 MiB | 105.2 MiB | +43.7 MiB |
| 8 GiB, 200% / 3x | 61.5 MiB | 236.7 MiB | +175.2 MiB |
| 16 GiB, 400% / 3x | 169.1 MiB | 946.7 MiB | +777.6 MiB |

The last row is the extreme: a 16 GiB machine at 400% zoom on a 3x display now
holds ~947 MiB of page texture rather than 169 MiB, because its survival ceiling
is 1408 MiB and nothing forces the reduction. That is a deliberate consequence of
the decision and it is reported rather than silent — but it is the row most worth
revisiting if real-device measurement shows driver-side overhead materially above
the requested bytes.

### What this bought

Every ordinary operating point on every device class now renders visible pages at
full resolution, and the bench asserts it directly at **every scroll offset of the
traversal** rather than at one position:

- a visible tile is at scale 1.0 unless `survival_reduced` is set;
- nothing exceeds the survival ceiling, on any device, at any offset;
- visible tiles are still never dropped, unchanged from r14.

The swept invariant also runs as a unit test across five memory sizes x four
zoom/DPI combinations x forty scroll offsets, so the property is pinned
independently of the bench.

### The ceiling also needs an absolute cap, not only a divisor

`available / 8` is purely proportional, and proportional has no opinion about
absurdity. A 64 GiB workstation reporting ~50 GiB available derives a **6.4 GiB**
texture ceiling for a word processor, and nothing else in the policy objects. The
byte *target* has had a floor and a ceiling clamp since T2.1 for exactly this
reason; the survival line had only a divisor.

`SURVIVAL_CAP_BYTES` is 1 GiB. The justification is not "enough pixels for the
screen" — it is far more than that, deliberately. It is the point past which the
extra bytes are overwhelmingly **page area that is not on screen**: tiles are
whole pages, so a page counts as visible when any part of it overlaps the
viewport, and at 400% zoom a US Letter page is ~12,700 device pixels tall on a 3x
display against a ~1,800 px viewport. Roughly 85% of a "visible" page's texture is
off-screen at any moment. Raising the ceiling past 1 GiB spends memory almost
entirely on page area the reader is not looking at — which is exactly what the
target exists to avoid spending on.

The cap changes **no measured row at 400% or below**, which is why the sweep was
extended to 600% — T5.4's planned zoom ceiling — rather than leaving a policy
whose only evidence is the test that asserts it:

| device | 600% / 1x | 600% / 2x | 600% / 3x |
| --- | ---: | ---: | ---: |
| workstation 64 GiB (cap 1024 MiB) | 236.7 | 946.7 `*` | **1024.0 `!`** (from 2130.0) |
| desktop 16 GiB (ceiling 1024 MiB, capped) | 236.7 | 946.7 `*` | 1024.0 `!` |
| design floor 8 GiB (ceiling 512 MiB) | 236.7 `*` | 512.0 `!` | 512.0 `!` |

Without the cap the workstation row would read 2130.0 MiB — 2.1 GiB of page
texture, held at full resolution, on the grounds that the machine could afford it.

**Sub-page tiling is the real fix for this regime and is not in Phase 2.**
Rasterising the visible *band* of a page rather than the whole page would make
visible residency proportional to viewport area — bounded by the display and
independent of zoom — instead of to page area, which would dissolve both the cap
and most of the survival regime. It is a change to the whole-page `PageTile`
abstraction rather than to this policy. `TODO(subpage-tiling)`.
