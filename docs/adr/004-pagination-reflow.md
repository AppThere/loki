# ADR 004: Pagination Reflow and Paragraph Splitting

## Status

Accepted

## Date

2026-04-20

## Context

`docs/adr/004-pagination-audit.md` documents the confirmed state of the
pagination pipeline as of this date. The key findings driving this ADR are:

**Current behaviour:**

1. `flow_section` (loki-layout/src/flow.rs) is the sole location of pagination
   logic. It places whole paragraphs: if a paragraph does not fit on the current
   page it is pushed intact to the next page. No lines are ever split across a
   page boundary.

2. `flow_section` correctly builds `LayoutPage` objects in `FlowState.pages`
   during its paginated pass, then **discards them** — flattening all pages into
   a single `Vec<PositionedItem>` with each page's items translated by
   `i × page_h + margins.top`.

3. `layout_document` (loki-layout/src/lib.rs, paginated branch) receives that
   flat list and **re-bins** items into `LayoutPage` objects using the boundary
   `y ∈ [i × page_h, (i+1) × page_h)`. This ignores the `margins.top` offset
   that `flow_section` already applied, so any item with `item_y < margins.top`
   lands in the wrong page bin.

4. `ParagraphLayout` (loki-layout/src/para.rs) discards Parley's per-line
   metrics after shaping; only `height`, `width`, `items`, `first_baseline`, and
   `last_baseline` are preserved. There is no way to find the split point between
   two page-fragments of a paragraph after `layout_paragraph` returns.

5. `keep_with_next` and `keep_together` are parsed into `ResolvedParaProps` and
   stored in `FlowState.pending_keep_with_next` but are never acted upon
   (`#[allow(dead_code)]`).

6. Parley 0.8 provides no native line-subset rendering API. Splitting output
   into page fragments must be implemented on top of the existing
   `Vec<PositionedItem>` output.

**Why it needs to change:**

Multi-page documents currently show every paragraph starting at a page boundary
but never continuing mid-paragraph across a page. Long paragraphs that exceed a
single page height are rendered partly off the bottom of the last page. The
double-pagination in `layout_document` also contains a correctness bug that
misplaces content near the top margin of each page.

---

## Decisions

### 1. Surface LayoutPage instead of re-binning

**Decision:** Change `flow_section` to return its internal `Vec<LayoutPage>`
directly in paginated mode instead of flattening. `layout_document`'s paginated
branch removes the re-binning loop entirely and uses those pages as-is.

**Mechanism:** Introduce a `FlowOutput` enum as the return type of `flow_section`:

```rust
pub enum FlowOutput {
    /// Returned when mode.is_paginated().  Pages are already page-local:
    /// item origins are relative to the page content-area top-left (0, 0).
    Pages {
        pages: Vec<LayoutPage>,
        warnings: Vec<LayoutWarning>,
    },
    /// Returned for Pageless and Reflow modes.
    Canvas {
        items: Vec<PositionedItem>,
        height: f32,
        warnings: Vec<LayoutWarning>,
    },
}
```

`flow_section`'s existing flat-translation block (flow.rs:133–154) is replaced
by:

```rust
if mode.is_paginated() {
    finish_page(&mut state);
    FlowOutput::Pages { pages: state.pages, warnings: state.warnings }
} else {
    // existing canvas path unchanged
    FlowOutput::Canvas { items: state.current_items, height: state.cursor_y,
                         warnings: state.warnings }
}
```

`layout_document` (Paginated branch) becomes:

```rust
let FlowOutput::Pages { pages, .. } = flow_section(…, &LayoutMode::Paginated, …)
    else { unreachable!() };
// Accumulate pages, renumber page_number relative to global_page_count.
```

**Bug fixed as a consequence:** The `margins.top` offset error described in
audit §B.3 is eliminated because the re-binning pass is removed. `flow_section`
already places items at page-content-area-relative coordinates (origin (0,0) at
the top of the content area); this is now the authoritative representation.

**No change to `PaginatedLayout` or `LayoutPage`** — these types are already
correct; the bug was in `layout_document`'s transformation of the data.

---

### 2. Paragraph splitting: Option A — Vello push_layer clip rect

**Decision:** Use Option A (Vello `push_layer` clip rect) for the initial
implementation.

**Chosen approach:** When a paragraph must be split at line boundary *k*, both
page fragments reuse the *same* `Vec<PositionedItem>` (cloned once). Each
fragment is wrapped in a new `PositionedItem::ClippedGroup` variant that carries
a `clip_rect: LayoutRect`. `loki-vello` renders a `ClippedGroup` by calling
`scene.push_layer` (Vello's blend/clip layer API), painting the child items, and
then calling `scene.pop_layer`.

Fragment A (current page, lines 0..k):
```
clip_rect = LayoutRect { x: 0, y: cursor_y, w: content_width, h: split_y }
items = all para items (translated to cursor_y as normal)
```

Fragment B (next page, lines k..n):
```
clip_rect = LayoutRect { x: 0, y: 0, w: content_width, h: para_height - split_y }
items = all para items (translated up by -split_y, placed at top of next page)
```

The clip rect on each fragment masks lines that belong to the other page. Both
background (`FilledRect`) and border (`BorderRect`) items — which span the full
paragraph height — are handled automatically by the clip without special-casing.

**Why not Option B (y-range item filtering):**

Option B filters each `PositionedItem` by its y-coordinate and discards items
outside the page window. This avoids rendering clipped content to the GPU, but
requires:

1. Extending `ParagraphLayout` with `line_boundaries` (needed to find split_y).
2. Replacing full-height `FilledRect`/`BorderRect` items with height-trimmed
   copies per fragment — these items have origin `(0, 0)` and height equal to
   the whole paragraph, so a simple y-range filter would incorrectly exclude
   them from both fragments.
3. Writing and testing the trimming logic for every `PositionedItem` variant.

Option A achieves the same visual result with none of those complications.
The GPU cost of rendering a few clipped lines is negligible for word-processor
page sizes.

**Option B remains a valid future optimisation** if profiling shows the GPU
cost matters (e.g., very dense pages with many split paragraphs). At that point
the `line_boundaries` field can be added to `ParagraphLayout` and the filter
path implemented. This is marked `TODO(split-optimise)` in the implementation.

**New type required:**

```rust
// loki-layout/src/items.rs — add to PositionedItem (#[non_exhaustive])
ClippedGroup {
    /// Clip rectangle in page-content-area coordinates (same space as the
    /// child items' origins).
    clip_rect: LayoutRect,
    /// Items to render inside the clip.
    items: Vec<PositionedItem>,
},
```

`loki-vello/src/scene.rs paint_items` gains an arm:

```rust
PositionedItem::ClippedGroup { clip_rect, items } => {
    let clip = clip_rect.translated(offset.0, offset.1);
    scene.push_layer(BlendMode::default(), 1.0, Affine::IDENTITY,
                     &kurbo::Rect::new(clip.x() as f64, clip.y() as f64,
                                       (clip.x() + clip.width()) as f64,
                                       (clip.y() + clip.height()) as f64));
    paint_items(scene, items, font_cache, offset, scale);
    scene.pop_layer();
}
```

---

### 3. Split point calculation

**Finding the split line:**

`ParagraphLayout` already holds `line_boundaries: Vec<(f32, f32)>` after
Decision 6 is implemented (see §6). Each entry is `(min_coord, max_coord)` for
that line in paragraph-local coordinates (layout units; origin at paragraph top).

```
remaining = page_content_height - cursor_y

split_k = largest k such that line_boundaries[k].1 <= remaining
        = (0..line_count).rev().find(|&k| line_boundaries[k].1 <= remaining)
split_y = line_boundaries[split_k].1   // bottom of last fitting line
```

**Three edge cases:**

| Case | Condition | Action |
|---|---|---|
| No lines fit | `split_k` does not exist (even `line_boundaries[0].1 > remaining`) and `cursor_y > 0` | Flush current page, reset `cursor_y = 0`, retry placement on the fresh page. If `cursor_y == 0` and still no lines fit, fall through to the "too tall" case. |
| Entire paragraph fits | `split_k == line_count - 1` (last line fits) | No split; place normally with existing code. |
| Paragraph taller than a full page | `para_layout.height > page_content_height` | Emit `LayoutWarning::BlockExceedsPageHeight`. Split at `split_y = page_content_height` (or the last fitting line boundary ≤ `page_content_height`). Place Fragment A, flush page, place Fragment B — which may itself exceed the next page and require another split. Loop until the remaining fragment height ≤ `page_content_height`. |

For the "no lines fit, cursor_y > 0" retry: after flushing, re-apply
`space_before` and attempt the split calculation once more against the full
`page_content_height`. If the paragraph still does not fit (height >
page_content_height), the "too tall" branch handles it.

---

### 4. keep-with-next and keep-together

#### keep-together

A paragraph with `keep_together: true` is treated as an atomic block.

**Algorithm:**

1. Compute `needed = para_layout.height + resolved.space_after`.
2. If `needed <= available` → place normally (already fits).
3. If `needed > available` and `cursor_y > 0` → flush page, reset cursor,
   re-compute `available = page_content_height`, retry step 2.
4. If after flushing `needed > page_content_height` → the block cannot be
   kept together on any page. Override: proceed with normal splitting (Decision
   3 algorithm). Emit:
   ```rust
   tracing::warn!(
       block_index,
       block_height = para_layout.height,
       "keep-together paragraph exceeds page height; splitting"
   );
   ```

Step 3 is identical to the existing "push whole paragraph to next page" logic;
the only addition is the override in step 4 (currently these blocks are placed
overflowing the page without splitting at all).

#### keep-with-next

A paragraph with `keep_with_next: true` must start on the same page as the
paragraph that immediately follows it.

**Algorithm (forward-scan, no buffering required):**

Before placing block *i* with `keep_with_next: true`:

1. Scan forward to find the end of the keep-with-next chain: the first
   subsequent block *j* where `keep_with_next == false` (or end of section).
2. Cap the chain at 5 blocks total (`j - i ≤ 5`). If exceeded, truncate at
   block `i + 5` and emit:
   ```rust
   tracing::warn!(start_block = i, "keep-with-next chain exceeds 5; truncating");
   ```
3. Sum the heights of blocks *i* through *j* (inclusive):
   `chain_height = Σ (para_layout.height + space_after)` for each block.
4. If `chain_height <= available` → the entire chain fits on the current page.
   Place blocks *i* through *j* normally, one after another.
5. If `chain_height > available` and `cursor_y > 0` → flush current page first,
   then place all chain blocks on the new page starting from *i*.
6. If `chain_height > page_content_height` → the chain cannot fit on one page
   even after flushing. Break the chain at the last block *k* where the prefix
   `i..=k` fits on a fresh page. Emit:
   ```rust
   tracing::warn!(start_block = i, end_block = j,
                  "keep-with-next chain too tall for one page; breaking at block {k}");
   ```

**Implementation note:** This forward-scan requires `layout_paragraph` to be
called speculatively for the lookahead blocks. To avoid double-layout, cache the
`ParagraphLayout` for each block during the scan and reuse it during placement.
This cache is local to `flow_section`'s main loop iteration.

**`FlowState` change:** `pending_keep_with_next` (currently dead code) is
replaced by the forward-scan approach above; the field is removed.

---

### 5. Output types

No new top-level types are needed. The changes are:

**`flow_section` return type** — new `FlowOutput` enum (see Decision 1):

```rust
// loki-layout/src/flow.rs
pub enum FlowOutput {
    Pages    { pages: Vec<LayoutPage>,       warnings: Vec<LayoutWarning> },
    Canvas   { items: Vec<PositionedItem>,   height: f32,
               warnings: Vec<LayoutWarning> },
}
```

**`PositionedItem` — new variant** (see Decision 2):

```rust
// loki-layout/src/items.rs  (#[non_exhaustive] enum)
ClippedGroup {
    clip_rect: LayoutRect,
    items: Vec<PositionedItem>,
},
```

`PositionedItem::translate` gains an arm for `ClippedGroup`:

```rust
Self::ClippedGroup { clip_rect, items } => {
    clip_rect.origin.x += dx;
    clip_rect.origin.y += dy;
    for item in items { item.translate(dx, dy); }
}
```

**`ParagraphLayout` — new field** (see Decision 6):

```rust
// loki-layout/src/para.rs
pub struct ParagraphLayout {
    pub height: f32,
    pub width: f32,
    pub items: Vec<PositionedItem>,
    pub first_baseline: f32,
    pub last_baseline: f32,
    /// (min_coord, max_coord) per line in paragraph-local layout units.
    /// Empty for empty paragraphs.  Used by flow_section for split-point
    /// calculation.
    pub line_boundaries: Vec<(f32, f32)>,
}
```

**`LayoutWarning` — new variant:**

```rust
// loki-layout/src/flow.rs
pub enum LayoutWarning {
    BlockExceedsPageHeight { block_index: usize, block_height: f32 }, // existing
    UnresolvedImage { src: String },                                   // existing
    /// keep-together override: block was split because it exceeds page height.
    KeepTogetherOverride { block_index: usize, block_height: f32 },
    /// keep-with-next chain truncated at chain_limit.
    KeepWithNextChainTruncated { start_block: usize, chain_length: usize },
    /// keep-with-next chain broken because it is too tall to fit on one page.
    KeepWithNextChainTooTall { start_block: usize, break_at: usize },
}
```

All existing public types (`PaginatedLayout`, `LayoutPage`, `ContinuousLayout`,
`DocumentLayout`) are **unchanged**.

---

### 6. ParagraphLayout extension

**Decision:** Add `line_boundaries` to `ParagraphLayout` now, even though
Option A does not strictly require it for rendering.

**Rationale:** `line_boundaries` is required to find the clean split point
(Decision 3). Without it, the only split point available would be a raw
`remaining` y-value that might land mid-line, producing a visually incorrect
clip (half a line visible at the page bottom). `line_boundaries` is collected
from Parley at zero additional shaping cost — Parley already computed
`min_coord`/`max_coord` as part of line breaking.

**What is added:**

```rust
// loki-layout/src/para.rs  — inside layout_paragraph, after break_all_lines:
let line_boundaries: Vec<(f32, f32)> = layout
    .lines()
    .map(|l| (l.metrics().min_coord, l.metrics().max_coord))
    .collect();
```

Returned in `ParagraphLayout { …, line_boundaries }`.

**What is deferred (TODO(split-optimise)):**

Option B's y-range item filter, which would avoid rendering clipped content to
the GPU. This optimisation requires `line_boundaries` (now available) plus the
per-item y-range filter and `FilledRect`/`BorderRect` trimming logic. Deferred
to a future session once the Option A baseline is stable and profiled.

---

## Implementation order

Each step must leave `cargo check --workspace` passing before the next begins.

1. **Add `line_boundaries` to `ParagraphLayout`** (`loki-layout/src/para.rs`).
   Populate it from Parley line metrics in `layout_paragraph`. Update all
   construction sites (tests that construct `ParagraphLayout` directly need the
   new field).

2. **Add `PositionedItem::ClippedGroup`** (`loki-layout/src/items.rs`). Add
   `translate` arm. The variant is unused at this point; `paint_items` in
   `loki-vello` silently ignores it via the existing `_ => {}` guard.

3. **Add `FlowOutput` and change `flow_section` return type**
   (`loki-layout/src/flow.rs`). Update `layout_document` to destructure
   `FlowOutput::Pages` in the paginated branch. Remove the re-binning loop.
   Update the continuous branch to destructure `FlowOutput::Canvas`. All call
   sites must compile; no behaviour changes yet.

4. **Implement paragraph splitting in `flow_paragraph`**
   (`loki-layout/src/flow.rs`). Use `line_boundaries` to find `split_k`.
   Emit `ClippedGroup` fragments. Handle the three edge cases from Decision 3.
   Add `KeepTogetherOverride` warning variant.

5. **Handle `ClippedGroup` in `loki-vello`** (`loki-vello/src/scene.rs`).
   Add `push_layer` / `pop_layer` arm to `paint_items`. Add `translate` arm
   to `translate_item`.

6. **Implement `keep_with_next` forward-scan** (`loki-layout/src/flow.rs`).
   Remove dead `pending_keep_with_next` field. Add chain-limit logic and
   new `LayoutWarning` variants.

7. **Implement `keep_together`** (`loki-layout/src/flow.rs`). Override
   behaviour for blocks taller than a full page. Add `KeepTogetherOverride`
   warning.

8. **Update and extend tests**: `flow_tests.rs`, `para_tests.rs`, and
   `loki-vello` scene tests.

---

## Out of scope

- Widow/orphan control (first/last line stranded alone on a page)
- Table row splitting across page boundaries
- Footnote reflow
- Right-to-left pagination
- Header and footer content (`header_items` / `footer_items` on `LayoutPage`
  remain empty; tracked separately)
- Columns

---

## Consequences

**What changes:**

- `flow_section` gains a new return type (`FlowOutput`). Any caller outside
  `loki-layout` that pattern-matches the current tuple return must be updated.
  At the time of writing, the only caller is `layout_document` in
  `loki-layout/src/lib.rs`.

- `ParagraphLayout` gains `line_boundaries: Vec<(f32, f32)>`. Tests that
  construct `ParagraphLayout` directly (e.g., in `flow_tests.rs`) must add the
  field (can default to `vec![]` for tests that do not exercise splitting).

- `PositionedItem` gains `ClippedGroup`. The `#[non_exhaustive]` attribute
  already forces all `match` arms in downstream crates to include a wildcard;
  no downstream breakage.

- `LayoutWarning` gains three new variants. Existing exhaustive matches in tests
  must be updated.

- The `margins.top` offset bug in `layout_document` is fixed. Documents that
  previously rendered with top-of-page content incorrectly placed on the
  preceding page will now render correctly. This is a behaviour change but
  purely a bug fix.

**What the implementation session will do differently:**

`flow_paragraph` will split long paragraphs at line boundaries instead of
pushing them whole to the next page. The output of `layout_document` (Paginated)
will contain correct, non-overlapping page assignments for all content.

---

## Open questions

1. **Vello `push_layer` clip API**: `vello::Scene::push_layer` takes a
   `BlendMode`, alpha, `Affine`, and a `peniko::Fill` shape. The clip shape
   must be constructed as a `kurbo::Rect`. Verify that Vello 0.6 (the version
   in `loki-vello/Cargo.toml`) exposes this API before step 5 of the
   implementation. If the API differs, the `ClippedGroup` rendering step may
   need adjustment.

2. **`keep_with_next` lookahead and `layout_paragraph` cost**: The forward-scan
   calls `layout_paragraph` speculatively for lookahead blocks. For typical
   documents this is a small number of paragraphs (headings followed by body).
   If sections contain very long keep-with-next chains this could be slow. A
   height-estimate heuristic (font size × estimated line count) could short-
   circuit the scan before committing to a full layout.

3. **Multi-split paragraphs and fragment ordering**: When a paragraph requires
   more than two fragments (height > 2 × page_content_height), each page holds
   one `ClippedGroup`. The clip rect for fragment *m* is
   `[m × page_content_height, (m+1) × page_content_height)` intersected with
   the paragraph's `line_boundaries`. Confirm that `finish_page` is called
   correctly between fragments and that page numbering is contiguous.

4. **`LayoutRect` origin convention**: `LayoutRect` is used throughout
   `loki-layout` with `origin` as top-left. The `clip_rect` on `ClippedGroup`
   must be in page-content-area-local coordinates at the point it is created in
   `flow_paragraph`. When `paint_items` in `loki-vello` applies the `offset`
   translation, it must translate the `clip_rect` origin by the same `(dx, dy)`
   before passing it to Vello. Confirm that `translate_item`'s `ClippedGroup`
   arm (Decision 5) translates `clip_rect.origin` in addition to child items.
