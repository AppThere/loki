# Incremental paginated layout

Status: **Stage 1 landed** (wired into the editor). Owner: layout. Guardrail:
`loki-acid/examples/relayout_bench.rs` + the `incremental == full` property
test in `loki-layout`. Measured ~13× faster per keystroke at 257 pages
(10.6 ms → 0.85 ms) for a height-preserving single-block edit.

## Problem

Every edit re-runs `layout_document` over the whole document. Shaping is already
memoised per paragraph (`para_cache`), so the remaining cost is the
flow/pagination pass: it walks every block, re-places it, and rebuilds the full
positioned-item list and editing index. Measured at ~7 µs/block (see
`relayout_bench`), i.e. linear in document size — ~10 ms/keystroke at ~260 pages,
~20 ms at ~500. The work is wasted because one keystroke changes one paragraph.

## Goal

Make a single-block content edit cost O(changed pages) instead of O(document).
Most keystrokes do not change a paragraph's **height** (line count), so the page
it sits on is the only page that must be rebuilt; every other page is reusable
verbatim.

## Contract (mirrors `loro_bridge::IncrementalReader`)

The incremental driver returns `Some(layout)` only when it can prove the result
is byte-identical to a full layout; otherwise it returns `None` and the caller
runs `layout_document`. Stage 1 fast-path eligibility:

- exactly one section (multi-section → full);
- `preserve_for_editing` set (the editing path);
- the document contains **no footnotes/endnotes** (they render at section end, so
  a content change can renumber/repaginate the whole tail → full);
- the change is a **content edit to existing blocks** — same block count, the
  dirty set is non-empty and in range (insert/delete/move → full);
- the previous layout carried reuse metadata (see below).

Coverage expands in later stages *behind the property test* — never by relaxing
the equality guarantee.

## Mechanism: clean-page-top checkpoints + prefix/suffix reuse

A page always starts at content-area top (`cursor_y == 0`, no items placed yet).
A **clean page top** is one that falls *between* top-level blocks (not inside a
split paragraph, table, or keep-with-next chain). At each clean page top during a
full paginated flow we record a checkpoint:

```text
PageStart { page_index, block_index, FlowCheckpoint }
FlowCheckpoint { page_number, list_counters, prev_list_id, note_counter, indent }
```

`FlowCheckpoint` is the entire resumable flow state at a clean page top (`cursor_y`
is 0 and the item/paragraph accumulators are empty by definition). Recording is
additive — it does not change layout output.

Incremental relayout of `doc` given the previous layout + checkpoints + previous
blocks:

1. **Diff** — first changed block `c` (min dirty index, or PartialEq diff).
2. **Prefix reuse** — let `pp` be the last checkpoint with `block_index ≤ c`.
   Pages `[0, pp.page_index)` cannot depend on block `c`; reuse them verbatim.
   Rebuild a `FlowState` from `pp.FlowCheckpoint` and resume the top-level flow at
   block `pp.block_index`.
3. **Re-flow** forward, recording new clean-page-top checkpoints.
4. **Suffix reuse (resync)** — at each new clean page top `(block_index b', state
   S')`, if the old metadata has a checkpoint with the same `b'` **and** equal
   `S'` **and** blocks `[b'..]` are unchanged, the remaining old pages are
   identical: append them (renumbering `page_number`) and stop. A height-unchanged
   edit resyncs at the next page top → O(1 page) re-flow.
5. **Headers/footers** — run the existing pass-2 `assign_headers_footers` over the
   full assembled page list (cheap; handles PAGE/NUMPAGES correctly when the page
   count changes).

If no resync occurs, the tail is recomputed — still correct, just not saved.

## Correctness gate

`incremental(prev, edited) == full_layout(edited)` is asserted by a property test
over a battery of edits (insert/delete char, same-height replace, line-growing
replace, edits at start/middle/end). Equality is compared structurally (page
count + per-page item Debug), the same exactness `para_cache` already relies on.
Any divergence fails the build; the driver is never allowed to ship a wrong page.

## Staging

- **Stage 1**: single-section, footnote-free, single-block content edit; prefix +
  suffix page reuse; full fallback for everything else.
- **Stage 2+**: footnote-bearing docs (section-end resync), multi-block edits,
  mid-split resume — each added only with the property test extended first.
