# Memory Audit — Loki Text (2026-06-12)

Investigation into high RAM usage (reported climbing past 1 GB with a few
10–20 page documents). This is a **code-level audit** — the headless CI
environment has no GPU, so figures below are derived from the data structures
and allocation sites, not a live profiler. Definitive per-allocation attribution
needs a profiler run on a real device (Windows / Pixel 9); the structural wastes
identified here are independent of that.

## Summary of findings

| # | Finding | Severity | Status |
|---|---------|----------|--------|
| 1 | Page tiles default to **Hot tier** and only demote on scroll-settle, so an opened-but-unscrolled document keeps a full-resolution texture per page | **Critical** | **Fixed** |
| 2 | No page virtualization — every page is a mounted tile holding a texture | High | Recommended |
| 3 | Inactive-tab sessions retain the full preserved layout (+ Loro + undo) | Medium | Recommended |
| 4 | Paragraph shaping cache accumulated across documents; generous cap | Medium | **Fixed** |
| 5 | Per-tile `FontDataCache` duplicates interned font bytes across tiles | Low | Recommended |
| 6 | Loro oplog grows with edit history and never compacts | Low | Recommended |

## Finding 1 — All pages render at Hot tier until you scroll (FIXED)

`loki-renderer` renders each page through a `LokiPageSource` whose texture is
sized by a cache **tier**: Hot = full scale (~12 MB for A4 at 2× DPR), Warm =
0.5× (~3 MB), Cold = 0.25× (~0.75 MB). Tiers are assigned by `assign_tier`
relative to the scroll viewport (Hot = within ~2× viewport height, Warm beyond
that, Cold = the rest).

The problem: `assign_tier` runs **only** inside `RendererState::on_settle`,
which the scroll-settle detector calls **only after a scroll gesture ends**.
There was no initial retier. So:

- On open (no scroll), the tier cache is empty.
- `LokiPageSource::render` defaults an un-tiered page to **Hot**
  (`page_paint_source.rs`, `.unwrap_or(CacheTier::Hot)`).
- Every page that gets painted allocates a **full-resolution** texture.

A 20-page document opened and read top-to-bottom (without a settling scroll
pause) holds up to ~20 × 12 MB ≈ **240 MB** of page textures — and this scales
with document length, the opposite of what's wanted.

**Fix** (`document_view.rs`): when the layout generation changes, schedule one
retier (`spawn(async { renderer.on_settle() })`, guarded to run once per
generation). This reuses the *proven* demotion path — the same code a scroll
already triggers — so pages outside the hot/warm zone are demoted to Cold
immediately on load. Resident texture memory becomes bounded by the viewport
neighbourhood (~2–3 Hot pages + a few Warm) regardless of document length:
~240 MB → ~45 MB for that 20-page document. The demotion path already
unregisters and frees the old Hot texture when a tile re-renders at a smaller
tier, so the memory is actually returned.

> Needs on-device validation: the GPU paint path can't run headlessly here. The
> change only triggers existing logic earlier (equivalent to a zero-distance
> scroll), and visible pages stay in the Hot zone, so no visual regression is
> expected.

## Finding 4 — Shaping cache accumulation (FIXED)

The paragraph shaping cache (`para_cache`, added for the keystroke-latency work)
memoises `ParagraphLayout`s — each retaining glyph runs, byte-index maps sized
to the paragraph text, and a Parley `Layout` (a few KB to tens of KB each). Two
issues:

- It was shared per editor and **never cleared on document load**, so switching
  between documents accumulated every document's paragraphs.
- The cap (4096 × 2 generations) allowed a large ceiling.

**Fix**: added `FontResources::clear_paragraph_cache`, called from
`seed_layout_from_document` (the load path) so the cache starts fresh per
document; lowered the cap to 2048 (still covers ~80 pages, well past typical
use). The renderer's cache is left to bound itself by rotation — it must **not**
clear per `update_doc` (that fires every keystroke and would destroy the
incremental reuse), and in paginated mode it barely computes anyway (it reuses
the editor's layout — see the single-canonical-layout change).

## Recommended (not yet implemented — higher risk / cross-cutting)

These need the GPU/app running to validate, so they are written up rather than
applied blind.

- **Finding 2 — Virtualize page tiles.** `DocumentView` mounts a `PageTile`
  for *every* page (`for (idx, w, h) in pages` over `0..page_count`). Even Cold,
  each tile keeps a ~0.75 MB texture, a `FontDataCache`, and component state.
  Render only the tiles within (hot ∪ warm ∪ a margin) of the viewport and mount
  lightweight placeholders elsewhere. With Finding 1 fixed the per-tile cost is
  already much lower, but this removes it for off-screen pages entirely.

- **Finding 3 — Drop preserved layouts for inactive tabs.** `DocSession`
  stashes `paginated_layout: Arc<PaginatedLayout>` for every inactive tab. With
  `preserve_for_editing`, that is one Parley `Layout` + index maps per paragraph
  (~9 MB for a 20-page doc). Unsaved *edits* must survive (the Loro doc), but the
  *layout* can be dropped on stash and recomputed on reactivation.

- **Finding 5 — Share the render `FontDataCache`.** Each `LokiPageSource` owns
  its own `FontDataCache`, so interned font bytes are duplicated across page
  tiles. `DocPageSource` already holds a shared `font_cache`; route tiles
  through it (behind its mutex) to dedupe.

- **Finding 6 — Compact the Loro oplog.** The CRDT retains full operation
  history; a long editing session grows unbounded. Periodic checkpoint/GC (or
  dropping undo history beyond a bound) caps it.

## How to confirm on-device

Run with a memory profiler (e.g. `heaptrack` on Linux, Instruments on macOS, or
Android Studio's Memory Profiler) and watch:
1. Open one 20-page document, do **not** scroll → resident GPU texture memory
   should now be tens of MB, not hundreds.
2. Scroll to the bottom and back → memory should stay bounded (tiers demote).
3. Open several documents in tabs → only the active document should hold page
   textures; inactive tabs hold CPU state only.
