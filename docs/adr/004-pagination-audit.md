# Pagination reflow — audit findings

**Date:** 2026-04-20  
**Branch:** claude/add-odt-format-support-3h3AR  
**Scope:** Read-only audit. No production code changed.

---

## Part A — Parley 0.8 line metrics API

### Crate version

`parley = "0.8"` (resolved: `parley-0.8.0`)  
Source: `/root/.cargo/registry/src/…/parley-0.8.0/`

### API table

| Item | Type / method | Fields / notes | Units |
|---|---|---|---|
| Laid-out paragraph result | `Layout<B>` | `.len()` line count; `.height()` total height; `.width()` max line advance; `.lines()` iterator; `.get(i)` line by index; `.scale()` display scale | layout units |
| Per-line accessor | `Line<'a, B>` | `.metrics() -> &LineMetrics`; `.break_reason() -> BreakReason`; `.text_range() -> Range<usize>`; `.items() -> impl Iterator<Item=PositionedLayoutItem>` | — |
| Per-line metrics | `LineMetrics` | `ascent`, `descent`, `leading`, `line_height` (= ascent+descent+leading), `baseline` (offset from `min_coord`), `offset` (alignment horizontal shift), `advance` (full width incl. trailing ws), `trailing_whitespace`, `min_coord` (line top), `max_coord` (line bottom) | layout units |
| Line count after layout | `layout.len(): usize` or `layout.lines().len()` (ExactSizeIterator) | — | — |
| Measuring height of N lines | `layout.get(n-1).map(\|l\| l.metrics().max_coord).unwrap_or(0.0)` — `max_coord` of line *n-1* is the bottom edge of lines 0..n | layout units |
| Rendering only lines M..N | **No native API.** Must iterate `layout.lines().skip(M).take(N-M)` and collect their `items()` before calling `layout_paragraph`. Post-`layout_paragraph` splitting requires filtering `PositionedItem`s by y-coordinate band; see §A.3 below. | — | — |
| Soft vs hard line break | `BreakReason` enum: `None` (first line / no break), `Regular` (soft word-wrap), `Explicit` (hard `\n` break), `Emergency` (mid-word overflow wrap) | available on `Line::break_reason()` | — |

### A.1 — Iterating lines and y-extents

`Layout::lines()` returns an `ExactSizeIterator + DoubleEndedIterator` of `Line<'_, B>`.

Each `Line` exposes `metrics().min_coord` and `metrics().max_coord`:

```
line 0: min_coord=0.0  max_coord=14.4   (line height = 14.4 lu)
line 1: min_coord=14.4 max_coord=28.8
…
```

`min_coord` is the top of the line; `max_coord` is the bottom. For a paragraph
of *n* lines, `layout.get(n-1).unwrap().metrics().max_coord == layout.height()`.

`layout.line_for_offset(y_offset)` performs a binary search over `min_coord` /
`max_coord` to find the line containing a given y-position.

### A.2 — Units

All coordinates emitted by Parley are in **layout units**:

```
layout_unit = point × display_scale
```

`loki-layout` passes `display_scale: f32` through to `ranged_builder(…, display_scale, …)`
(see `loki-layout/src/para.rs:167`).  At `display_scale = 1.0` (test / headless
usage), 1 layout unit = 1 CSS/CSS3 point.

`PositionedGlyphRun.origin.y` is the **baseline** of the run, equal to
`line.metrics().baseline` (which itself equals `line.metrics().min_coord + ascent`
in the common case).  The top of the line is `metrics().min_coord`; the bottom is
`metrics().max_coord`.

### A.3 — Rendering a subset of lines: no native Parley support

Parley provides no `render_lines(M..N)` method.  The `Layout` object holds all
shaping results; there is no incremental rendering interface.

**Post-layout splitting** (the approach available to `loki-layout`):

After `layout_paragraph` produces a flat `Vec<PositionedItem>`:

- `PositionedGlyphRun.origin.y` is the baseline; a glyph run belongs to the line
  whose `min_coord <= baseline < max_coord`.
- `PositionedDecoration.y` follows the same baseline value.
- `PositionedRect` / `PositionedBorderRect` origins are at `(0, 0)` for
  paragraph-level backgrounds and borders — they span the full paragraph height
  and cannot be trivially split by y-filter.

To render a split fragment on a different page, one of two approaches is needed:

1. **Y-range item filter**: collect items whose `origin.y` (or `rect.y`) falls
   within `[split_top, split_bottom)` and translate them up by `-split_top`.
   Background and border rects must be replaced with height-trimmed copies.

2. **Vello push_layer clip**: emit all items for the fragment, wrapping them in
   `scene.push_layer(…, clip_rect, …)` / `scene.pop_layer()`.  This avoids
   special-casing backgrounds and borders but requires `loki-vello` to gain
   a new `PositionedItem::ClippedGroup` variant (or an explicit clip parameter
   on each paint call).

Neither approach exists today; both require new code.

### A.4 — BreakReason: soft vs hard breaks

```rust
pub enum BreakReason {
    None,       // first line — no preceding break
    Regular,    // soft word-wrap
    Explicit,   // hard \n / paragraph break character
    Emergency,  // overflow mid-word break
}
```

Available via `Line::break_reason()`.  Useful to detect the last line of a
logical paragraph when multiple `StyledParagraph` blocks share the same Parley
`Layout` (not the current model, but relevant if layout is ever unified).

---

## Part B — `layout_document` and `paint_layout` current structure

### B.1 — `layout_document` return type

`loki_layout::layout_document(resources, doc, mode, display_scale) -> DocumentLayout`

`DocumentLayout` is `#[non_exhaustive]`:

```rust
pub enum DocumentLayout {
    Paginated(PaginatedLayout),
    Continuous(ContinuousLayout),
}
```

`PaginatedLayout { page_size: LayoutSize, pages: Vec<LayoutPage> }`.

`LayoutPage { page_number, page_size, margins, content_items: Vec<PositionedItem>,
              header_items, footer_items }`.

`ContinuousLayout { content_width: f32, total_height: f32, items: Vec<PositionedItem> }`.

### B.2 — A single laid-out block

There is no "block" type in the output.  `layout_document` produces a flat
`Vec<PositionedItem>` per page (or per canvas).  Block boundaries are erased
during `flow_section`.

`PositionedItem` variants (`#[non_exhaustive]`):

| Variant | Key coordinate field | Meaning |
|---|---|---|
| `GlyphRun(PositionedGlyphRun)` | `origin: LayoutPoint` (x, y = baseline) | Shaped glyph run |
| `FilledRect(PositionedRect)` | `rect: LayoutRect` (x, y = top-left) | Background fill |
| `BorderRect(PositionedBorderRect)` | `rect: LayoutRect` | Paragraph/cell border |
| `Image(PositionedImage)` | `rect: LayoutRect` | Inline image |
| `Decoration(PositionedDecoration)` | `x, y` (baseline-relative) | Underline / strikethrough |
| `HorizontalRule(PositionedRect)` | `rect: LayoutRect` | `<hr>` line |

### B.3 — Page boundary logic

**`flow_section` (loki-layout/src/flow.rs)**

`flow_section` is the sole location of pagination logic:

- Maintains `FlowState { cursor_y, current_items, pages, page_content_height, … }`.
- `flow_paragraph` checks `needed = para_layout.height + space_after` vs
  `available = page_content_height - cursor_y`.
- If `needed > available && cursor_y > 0`: calls `finish_page()`, resets
  `cursor_y = 0`, re-applies `space_before`.
- **No per-line split** — the paragraph is pushed whole to the next page.
- `pending_keep_with_next: bool` exists in `FlowState` but is never set or read
  (`#[allow(dead_code)]`); `keep_with_next` and `keep_together` are parsed into
  `ResolvedParaProps` but not acted upon.
- At the end of paginated flow, `finish_page()` is called and the pages are
  **flattened** into a single `Vec<PositionedItem>` with each page's items
  translated by `i * page_h + margins.top` vertically.

**`layout_document` (loki-layout/src/lib.rs) — Paginated path**

`layout_document` calls `flow_section(…, &LayoutMode::Paginated, …)` and
receives the flattened `(items, _, _)` tuple.  It then **re-bins** items into
`LayoutPage` objects by raw y-range:

```rust
if y >= p_idx as f32 * page_h && y < (p_idx + 1) as f32 * page_h { … }
```

This double-processes pagination and contains a latent bug:

- `flow_section` translates page-*i* items to `y = i * page_h + margins.top + item_y`.
- The re-bin boundary is at `i * page_h` (ignoring `margins.top`).
- Items near the top of page *i* (with `item_y < margins.top`) fall into the
  wrong bin — they land in page *i-1* instead of page *i*.

`flow_section` already builds correct `LayoutPage` objects internally
(`FlowState.pages`), but `layout_document` throws them away by calling the
flat-return variant of `flow_section`.

**`loki-vello` — no pagination logic**

`paint_paginated` (scene.rs:147) iterates `PaginatedLayout.pages` and for each
page advances a `y_cursor` by `page_height + PAGE_GAP_PT`.  It calls
`paint_items(page.content_items, page_offset)` with `page_offset = (x, y_cursor)`.

`paint_items` translates each item's coordinates by the page offset before
dispatching to the type-specific painter.  There is **no clip rect** applied
around page content; items outside the page boundary are rendered without
masking.

### B.4 — What `paint_layout` receives and iterates

```
paint_layout(scene, layout, font_cache, offset, scale, page_index)
  → match layout {
      Paginated(pl) →  if page_index.is_some() → paint_single_page(pl, idx)
                       else                     → paint_paginated(pl)
      Continuous(cl) → paint_continuous(cl)
    }
```

For paginated output, `paint_items` iterates `page.content_items: &[PositionedItem]`
in order and dispatches:

| Item | Painter |
|---|---|
| `GlyphRun` | `crate::glyph::paint_glyph_run` |
| `FilledRect` | `crate::rect::paint_filled_rect` |
| `BorderRect` | `crate::rect::paint_border_rect` |
| `Image` | `crate::image::paint_image` |
| `Decoration` | `crate::decor::paint_decoration` |
| `HorizontalRule` | rendered as grey `FilledRect` |
| unknown (`_`) | silently ignored (non_exhaustive guard) |

No layer push/pop, no clip rect, no blend mode — each item is painted
unconditionally with a direct coordinate translation.

---

## Summary of gaps relevant to pagination reflow

| Gap | Location | Notes |
|---|---|---|
| No per-line metadata in `ParagraphLayout` | `loki-layout/src/para.rs` | `ParagraphLayout` exposes only `height`, `width`, `items`, `first_baseline`, `last_baseline`; per-line `min_coord`/`max_coord` are discarded after `layout_paragraph` returns |
| No paragraph splitting in `flow_paragraph` | `loki-layout/src/flow.rs:232` | Whole paragraph pushed to next page; no split at line boundary |
| `keep_with_next` / `keep_together` not implemented | `loki-layout/src/flow.rs:69` | Parsed into `ResolvedParaProps`; `pending_keep_with_next` is dead code |
| `layout_document` paginated path re-bins incorrectly | `loki-layout/src/lib.rs:82-138` | Discards `flow_section`'s `LayoutPage` objects, re-bins flat list by raw y; margin-offset error moves top-of-page items to wrong page |
| No clip mechanism in `paint_items` | `loki-vello/src/scene.rs:222` | Items that cross a page boundary are rendered without masking |
| No `ClippedGroup` / `push_layer` in `PositionedItem` | `loki-layout/src/items.rs` | Vello `push_layer` clip approach would require a new non_exhaustive variant |
