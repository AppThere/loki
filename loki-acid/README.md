# loki-acid — ACID rendering-fidelity test harness

Operationalises [`TEST_PLAN.md`](./TEST_PLAN.md): a catalog of office-document
constructs that alternative suites render differently from the canonical
Microsoft 365 (OOXML) / LibreOffice (ODF) render, plus the machinery to diff
Loki's output against golden references.

## What runs today (no GPU, no goldens)

The plan names **page-count drift** and **glyph coverage (no tofu / `.notdef`)**
as the cheap canaries to check before any per-pixel diff. Both come straight
from the layout, so they run in any environment:

```bash
cargo test -p loki-acid                       # structural canaries + unit tests
cargo run  -p loki-acid --example acid_report # JSON + human summary of every fixture
```

`acid_report` imports each fixture, paginates the word-processing documents, and
reports page counts, sheet counts, and glyph coverage. Example summary:

```
ACID fidelity report — 139 catalogued cases
  acid_docx.docx [          ok] 3 page(s) — 1234 glyphs, full coverage
  acid_odt.odt   [          ok] 2 page(s) — 567 glyphs, full coverage
  acid_xlsx.xlsx [          ok] 4 sheet(s)
  acid_ods.ods   [          ok] 2 sheet(s)
  acid_odp.odp   [ no importer] no importer yet for this ODF format
  acid_odg.odg   [ no importer] no importer yet for this ODF format
```

## Layers

| Module | Role |
|--------|------|
| `catalog` | Machine-readable transcription of every `TC-*` case (id, format, severity). |
| `fixtures` | The embedded acid documents (`include_bytes!`) + import dispatch. |
| `pages` | Pagination + glyph-coverage (tofu) analysis — the rasteriser-free canaries. |
| `diff` | SSIM and absolute-difference metrics over decoded RGBA images. |
| `golden` | Discovery of golden reference renders and Loki renders. |
| `report` | Per-fixture and aggregate structural reports (serialisable). |

## Fixtures & importer coverage

| Fixture | Format | Importer | Pipeline exercised |
|---------|--------|----------|--------------------|
| `acid_docx.docx` | DOCX | ✅ | import → paginate → glyph coverage |
| `acid_odt.odt`   | ODT  | ✅ | import → paginate → glyph coverage |
| `acid_xlsx.xlsx` | XLSX | ✅ | import → workbook (sheet count) |
| `acid_ods.ods`   | ODS  | ✅ | import → workbook (sheet count) |
| `acid_odp.odp`   | ODP  | ❌ | catalogued; awaiting an ODP importer |
| `acid_odg.odg`   | ODG  | ❌ | catalogued; awaiting an ODG importer |
| `acid_pptx.pptx` | PPTX | — | catalogued; **fixture not yet supplied** |

## The golden pixel diff (when references exist)

Per the plan: render in Loki at 150 DPI, render the same file in the canonical
authority, diff with perceptual SSIM plus the hard glyph-coverage check.

1. Produce canonical renders → `goldens/<stem>/page-NNN.png`
   (see [`goldens/README.md`](./goldens/README.md)).
2. Produce Loki renders → `renders/<stem>/page-NNN.png`.
3. `cargo test -p loki-acid golden_pages_match_within_ssim_threshold`
   pairs the trees and asserts mean SSIM ≥ 0.98.

Until both trees are populated the golden test is a documented no-op, so the
suite stays green. The SSIM/diff primitives in `diff` are pure and unit-tested,
so they are correct the moment pixels arrive.

### Why Loki renders aren't produced here yet

Loki's renderer is GPU-backed (Vello / wgpu); a headless rasteriser is not
available in every CI environment. The `renders/` tree is therefore populated by
an external step (a GPU-capable runner, or a future `vello_cpu` path). The
harness is structured so that wiring that step in is the only remaining work —
everything downstream of "two PNGs" already exists and is tested.

## ODF round-trip note

The plan also calls for an ODF structural round-trip (re-export and diff the
targeted XML elements). ODT export exists; ODS/ODP/ODG export is partial or
absent, so the structural round-trip is tracked as future work alongside the
ODP/ODG importers.
