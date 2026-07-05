<!--
SPDX-License-Identifier: Apache-2.0
-->

# Visual-axis calibration record (Spec 02 §7.4 / D5)

The threshold is **data, not folklore**: this record documents the measured
cross-renderer noise floor behind `Tolerance::calibrated()`
(`appthere_conformance::golden`). Update the constants only together with
this record.

| | |
|---|---|
| **Date** | 2026-07-05 |
| **Corpus** | `fixtures/odt/{para-carlito, styles-tinos, para-gelasio}.odt` — the M4 baseline set (single-page, metric-compatible font names per D4) |
| **Golden side** | LibreOffice 24.2 headless → PDF → pinned rasterizer (`pdftoppm 24.02.0`, 144 dpi, `-aa yes -aaVector yes`), per `scripts/generate-odf-goldens.sh` |
| **Candidate side** | `loki-odf` import → `loki-layout` (bundled faces registered) → `loki-render-cpu` (`vello_cpu` =0.0.9) at `CONFORMANCE_DPI` = 144 |
| **Fonts** | The bundled `loki-fonts` faces, installed for fontconfig so both sides shape the identical bytes |
| **Method** | `cargo run -p loki-render-cpu --example calibrate_odf` — 64 px regions, windowed SSIM + mean CIEDE2000 ΔE per region, pages cropped to the common area (the two sides differ by ≤1 px of DPI rounding) |

## Measured distributions (500 regions/fixture, 1500 total; re-measured 2026-07-05 after the kerning fix)

| Fixture | SSIM min | SSIM p1 | SSIM p5 | ΔE max | ΔE p99 | ΔE p95 |
|---|---|---|---|---|---|---|
| `styles-tinos` | 0.6603 | 0.8894 | 1.0000 | 7.854 | 1.112 | 0.000 |
| `para-gelasio` | 0.9044 | 0.9650 | 0.9985 | 4.014 | 2.336 | 0.177 |
| `para-carlito` | 0.6922 | 0.7875 | 0.9804 | 7.515 | 5.251 | 1.408 |

All three fixtures now sit in one noise band and pass the calibrated
thresholds — the visual golden suite (`loki-render-cpu/tests/visual_golden.rs`)
asserts all of them.

## Analysis — how the divergence was root-caused (kept as a worked example)

The first calibration run (2026-07-05, same day) measured `para-carlito` at
SSIM min **0.2348** / ΔE max **19.8** — structural divergence. Diagnosis went
through two corrections, both driven by this harness's own artifacts:

1. The heatmap showed per-line cumulative rightward drift, initially read as
   *loki missing kerning* (fidelity gap #23 was listed open). A shaping probe
   disproved that: Parley 0.10's harfrust shaper applies Carlito's GPOS kern
   pairs (and `fi` ligatures) — the historical gap had been closed silently
   by the Parley 0.8→0.10 upgrade.
2. Per-line ink measurement then showed the *golden* consistently ~4–7 px
   **wider** per line — the total kern amount, with the sign inverted from
   the first reading: **LibreOffice was not kerning this document** (the
   fixture ODT carries no `style:letter-kerning`, and both LO-on-import and
   Word default pair kerning off), while loki kerned unconditionally. The
   width delta flipped borderline line wraps, cascading into whole-line
   diffs.

**Fix (the real gap #23):** `CharProps.kerning` is now forwarded to layout
(`StyleSpan::kerning`) and applied as a shaper feature toggle with a
reference-matching default of **off** (`"kern" 0` unless the document enables
it). Regression-locked by `loki-layout/tests/kerning_applied.rs`: kern applied
when enabled, natural advances when defaulted, contextual pairs only,
ligatures unaffected.

## Decision

- **Calibrated thresholds:** `min_ssim = 0.60` (re-measured floor 0.6603),
  `max_delta_e = 10.0` (re-measured max 7.854). Confirmed against the
  post-fix distributions; unchanged from the initial derivation.
- **The visual axis now has all fixtures green** and can be considered for
  promotion to a hard CI gate once the corpus grows past this baseline set
  (the gate-hardening call is the maintainer's; the suite already fails a
  PR that regresses any fixture past tolerance).

## Reproduction

```sh
cp loki-fonts/fonts/*.ttf ~/.fonts && fc-cache
./scripts/generate-odf-goldens.sh
cargo run -p loki-render-cpu --example calibrate_odf
```
