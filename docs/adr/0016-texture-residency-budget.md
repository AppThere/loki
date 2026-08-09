<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0016 — Texture residency lives in `appthere-canvas`; the tiered render cache is superseded

| Field | Value |
| --- | --- |
| Status | **Accepted** — 2026-07-26 |
| Supersedes | The Hot/Warm/Cold GPU page-texture tiering (see §2) |
| Drivers | Spec 08 I-01, I-19, D-10, T2.4, T2.4a; ADR L08-002, L08-017 |
| Affects | `appthere-canvas`, `loki-renderer`; deletes `loki-render-cache` |

---

## 1. Context

Spec 08 r1 described `loki-render-cache` as holding Hot/Warm/Cold page-texture
tiers, and planned Phase 2 around re-tiering them. Spike S0.2 found the crate is
**115 lines** containing `PageIndex`, a `CacheKey` marker trait, a `PageSource`
trait and a `GpuTexture` struct — no cache, no tiers, no eviction. The tiering
had been removed and nothing recorded it.

Two of r1's false capability claims trace to that gap. A crate keeps asserting
what its name says long after the code stops, and an accepted decision
describing a capability nobody has deleted keeps that assertion respectable.
L08-017 is the rule this ADR discharges: **a crate deleted for not doing what it
claims requires its governing decision to be superseded in the same change.**

Nothing about this is hypothetical. Two live documents still asserted the tiers
existed at the time of writing:

- `docs/adr/spec-06-benchmarking-audit.md` listed "GPU page-texture tiers —
  `loki-renderer` (`CacheTier` Hot/Warm/Cold, `assign_tier`)" as a bounded
  collection, marked ✅ **Yes (post-fix)**. Neither `CacheTier` nor `assign_tier`
  exists in the tree.
- `docs/adr/spec-06-calibration.md` §"No GPU page textures" referred to "the
  tiered render cache (Hot/Warm/Cold, …)".

Both are corrected by this change.

## 2. Decision 1 — the tiering is superseded, not merely absent

**Resident page textures are bounded by a byte budget across the zoom × device
scale axis, not by a tier assignment.** Under budget pressure the response is
**rasterisation scale** for off-centre pages, never eviction of visible content
(Spec 08 L08-002, T2.2).

Why a budget rather than restoring tiers:

- **Tiers ranked pages; the problem is not ranking.** Textures are already
  windowed to the viewport neighbourhood (`visible_window`), and measurement
  confirms residency is byte-identical across 10-, 100- and 500-page documents.
  A tier assignment solves a document-length problem that no longer exists.
- **The axis that is unbounded is zoom × DPI**, which no ranking of pages
  addresses: at 200% on a HiDPI display three resident tiles cost 165 MB, and at
  400% on a 3× display two cost 947 MB. Fewer, larger tiles is the shape, so the
  lever is tile *size*.
- **Degrading resolution beats dropping content.** A page at reduced
  rasterisation scale is blurry until it settles; an evicted visible page is
  blank. R5 tracks the blur; the eviction alternative has no mitigation.

## 3. Decision 2 — `loki-render-cache` is deleted, its types move to `appthere-canvas`

`PageIndex`, `CacheKey`, `PageSource`, `RenderError` and `GpuTexture` move into
`appthere-canvas`, which already re-exported every one of them, so no call site
changes path. The workspace member, the dependency-direction layer entry and the
`appthere-canvas → loki-render-cache` edge all go.

`GpuTexture` gains private fields, a `new` constructor that records the
allocation, and a `Drop` that records the release. That is not tidying: a public
struct literal would allocate without recording, and the resulting counter drift
is invisible — residency reads low forever after, and the budget check that
eventually fails does so for an unrelated reason.

## 4. Decision 3 (T2.4a) — the budget lands in `appthere-canvas`

Deleting the cache left the budget without a home. T2.4a says to decide by where
allocation happens rather than by where it feels tidy, so here is what the tree
actually shows.

**Where allocation happens.** The one live call is
`loki-renderer/src/page_paint_render.rs::allocate_page_texture`, driven by
`LokiPageSource::render`. The standalone `impl PageSource for DocPageSource`
also allocates and has **no callers in the workspace**. So the `wgpu` call is in
`loki-renderer`.

**Where residency is decided.** Not at the allocation. Two things fix the byte
total, and the `wgpu` call is downstream of both: which pages mount
(`visible_window`) and how large each tile is (`pt × 96/72 × zoom × dsf`).

**Decision: `appthere-canvas`**, as `appthere_canvas::residency`, with
`loki-renderer` consuming it — `virtualize` re-exports `visible_window` rather
than keeping a copy. Reasons, in order of weight:

1. **It owns the types every allocation produces.** After §3, `GpuTexture` and
   `PageSource` are here; a budget over textures belongs with the texture types.
2. **A model in `loki-renderer` could not be measured.** The Phase 2 bench is
   headless and must not build Dioxus, Blitz and wgpu to compute integer
   arithmetic. Reachability from a bench is a design constraint, not a
   convenience, because a budget nobody can measure is a budget nobody can
   check.
3. **One implementation of the mounting rule.** Production and the model call
   the same function. A model that restated the rule would drift from it, and a
   drifted model measures itself.

**What this decision does *not* rest on.** T2.4a offers "or `appthere-canvas` if
residency is genuinely shared with Iris". That is **not established**: there is
no Iris crate in this workspace, so the claim cannot be checked, and L08-015
forbids building on it. `appthere-canvas` happens to be AppThere-scoped and
`loki-*`-free, so Iris *could* take it — but the decision above stands on the
three reasons given, not on that.

## 5. Consequences

- `docs/adr/0009-target-architecture.md`'s L4 row and layer table lose
  `loki-render-cache`; `appthere-canvas` loses its only internal dependency and
  becomes an L4 leaf.
- The two spec-06 documents that asserted the tiers are corrected in the same
  change, per L08-017.
- `Metric::TextureBytes` joins `loki-bench` on the **Portable** axis: the byte
  accounting is CPU-side arithmetic at the allocation sites. Driver-side
  overhead on top of it stays device-bound (Spec 08 R16, revised).
- The budget's *derivation* consumes `DeviceProfile` (system RAM, GPU class),
  which lives in `appthere-ui` at L5 — uphill of `appthere-canvas`. So the
  derivation takes plain numbers and the **application** supplies them. That
  keeps the arithmetic testable and the layering intact.

## 6. Alternatives considered

**Keep the crate and give it the residency logic.** S0.2 §7 proposed exactly
this — "a `PageResidency` type in `loki-render-cache`, the crate finally earning
its name". Rejected: it is the same mistake in the other direction. The name
would be earned by writing code to fit a name, and `appthere-canvas` already
held the types, the re-exports and the AppThere scope.

**Put the budget in `loki-renderer`, beside the `wgpu` call.** Rejected on
reason 2 above — it would put Phase 2's model behind the whole GPU stack, and
the measurement is the point.

**Restore Hot/Warm/Cold.** Rejected on §2: it ranks pages, and the unbounded
axis is not page count.
