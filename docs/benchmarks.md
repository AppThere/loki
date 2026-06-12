# Performance benchmarks

The `loki-layout` crate carries the workspace's layout / edit-path benchmark
harness. It exists to turn the performance assessment's *estimates* into
*measurements* — specifically to characterise how per-keystroke cost scales with
document length, which is the dominant performance risk for taking on Office
365.

All targets are GPU-free and run headlessly (no display, no Vulkan), so they
work in CI and on a dev box. They live in `loki-layout` because it is the
narrowest crate that depends on both the layout engine and the Loro bridge.

## What is measured

The editor's per-keystroke pipeline
(`loki_text::editing::state::apply_mutation_and_relayout`) does two
whole-document passes on every edit:

1. `loro_to_document` — reconstruct the entire `Document` from the CRDT.
2. `layout_document` — re-lay-out the entire document.

The harness reproduces both, plus the standalone `FontResources::new()` font
scan that the renderer path currently repeats per generation.

| Target | Kind | Measures |
|---|---|---|
| `benches/layout_scaling.rs` | Criterion | `layout_document` (Paginated + Reflow) vs paragraph count; `FontResources::new()` in isolation |
| `benches/edit_path.rs` | Criterion | full per-keystroke pipeline (`loro_to_document` + `layout_document`) vs paragraph count |
| `examples/layout_report.rs` | plain table | quick median readout of all of the above, with a µs/paragraph column |
| `benches/support/mod.rs` | shared | synthetic-document builder (`build_doc`), `#[path]`-included by the above |

A single shared `FontResources` is reused across iterations, matching the
editor's `shared_font_resources`, so the font scan is not charged per layout.

## Running

```sh
# Rigorous statistical numbers (slow):
cargo bench -p loki-layout --bench layout_scaling
cargo bench -p loki-layout --bench edit_path

# Quick human-readable scaling table (fast — good on a target device):
cargo run -p loki-layout --example layout_report --release

# Filter Criterion to a subset while iterating:
cargo bench -p loki-layout --bench layout_scaling -- 'paginated/100'
```

## Baseline (headless x86-64 dev container, release example)

Indicative only — absolute numbers are machine-dependent; the **shape** is the
finding. Re-run on your target hardware (Windows / Pixel 9) for device numbers.

### Before the shaping cache (whole-document re-shape per keystroke)

| paras | paginate | reflow | keystroke | µs/para |
|------:|---------:|-------:|----------:|--------:|
| 10    | 1.4 ms   | 1.4 ms | 1.5 ms    | ~155 |
| 100   | 15.3 ms  | 15.4 ms| 15.9 ms   | ~158 |
| 500   | 77.6 ms  | 79.6 ms| 83.3 ms   | ~166 |
| 1000  | 161 ms   | 160 ms | 166 ms    | ~166 |

Cost-per-paragraph was roughly flat, i.e. each keystroke re-shaped the whole
document — O(n).

### After the shaping cache (`para_cache`, re-shape only the changed paragraph)

| paras | paginate | reflow | keystroke | speed-up (keystroke) |
|------:|---------:|-------:|----------:|---------------------:|
| 10    | 0.13 ms  | 0.11 ms| 0.16 ms   | ~10× |
| 100   | 1.5 ms   | 1.4 ms | 1.8 ms    | ~9× |
| 500   | 8.7 ms   | 7.8 ms | 10.4 ms   | ~8× |
| 1000  | 16.4 ms  | 16.2 ms| 22.2 ms   | ~7.5× |

(Criterion confirms the 1000-paragraph keystroke at ~22 ms, down from ~166 ms.)

`FontResources::new()` ≈ 7 ms here (higher on systems with more installed
fonts) — previously paid on **every** generation by the renderer path; now paid
once (see below).

**Reading:** with shaping memoised, an edit re-shapes only the changed
paragraph; the other `N − 1` are served from cache. Layout is no longer the
dominant per-keystroke cost.

### After incremental reconstruction (`IncrementalReader`)

Replacing the full `loro_to_document` walk with a diff-driven re-derive of only
the changed block (`edit_path` bench, 1000-paragraph doc, Criterion):

| keystroke path | time @ 1000 paras |
|----|---:|
| `loro_to_document` + cached layout | ~17.7 ms |
| `IncrementalReader` + cached layout | ~14.0 ms |

The full CRDT walk is gone from the hot path (~3.7 ms / ~21 % off the
keystroke; the gap widens with document size). End to end across both Tier-0
commits, the 1000-paragraph keystroke went from **~166 ms → ~14 ms (~12×)**.
The residual is now `layout_document`'s per-paragraph positioning (cache-key
hash + `ParagraphLayout` clone + flow), i.e. the single-canonical-layout merge
is the next lever.

> The `edit_path` keystroke bench re-inserts the same character each iteration,
> so after warm-up the edited paragraph is also a cache hit; a real keystroke
> types a fresh character and pays one extra paragraph shape (~150 µs), which is
> immaterial against the totals above.

## What changed (Tier-0 fixes)

- **Paragraph shaping cache** (`loki-layout/src/para_cache.rs`): memoises
  `ParagraphLayout` inside the shared `FontResources`, keyed by a hash of every
  shaping input (text + `Debug` of spans/props + width + scale + preserve
  flag). Layout output is **byte-identical** — only re-computation is skipped.
- **Persistent renderer font/shaping context**
  (`loki-renderer/src/doc_page_source.rs`): `DocPageSource` now holds one
  `FontResources` for its lifetime instead of constructing a fresh one per
  generation, so the ~20 MB system-font scan happens once and the shaping cache
  survives across keystrokes on the render path too.
- **Incremental Loro→Document reconstruction**
  (`loki-doc-model/src/loro_bridge/incremental.rs`): `IncrementalReader` keeps
  the last-derived `Document` plus the Loro version it reflects, diffs the new
  version against the old, and re-reconstructs only the changed block(s) when
  the edit is confined to existing blocks in section 0. Structural changes
  (split/merge/insert/delete), section/layout/metadata changes, or any
  unmappable diff fall back to a full `loro_to_document`, so the result is
  always byte-identical to a full rebuild. Wired into
  `apply_mutation_and_relayout`; reset when a new document loads.
