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

| paras | paginate | reflow | keystroke | µs/para |
|------:|---------:|-------:|----------:|--------:|
| 10    | 1.4 ms   | 1.4 ms | 1.5 ms    | ~155 |
| 100   | 15.3 ms  | 15.4 ms| 15.9 ms   | ~158 |
| 500   | 77.6 ms  | 79.6 ms| 83.3 ms   | ~166 |
| 1000  | 161 ms   | 160 ms | 166 ms    | ~166 |

`FontResources::new()` ≈ 7 ms here (higher on systems with more installed
fonts).

**Reading:** cost-per-paragraph is roughly flat across document sizes, so each
keystroke recomputes the whole document — cost is O(n) in document length. At
~1000 paragraphs (~40 pages) a single keystroke already costs ~166 ms, versus
the ~10 ms target for a fluid editor. The Loro traversal adds only a few ms on
top of layout, so `layout_document` is the primary cost centre and the first
target for the incremental-layout work (Tier 0 of the assessment roadmap).
