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

## Measured distributions (500 regions/fixture, 1500 total)

| Fixture | SSIM min | SSIM p1 | SSIM p5 | ΔE max | ΔE p99 | ΔE p95 |
|---|---|---|---|---|---|---|
| `styles-tinos` | 0.6324 | 0.8894 | 1.0000 | 9.083 | 1.112 | 0.000 |
| `para-gelasio` | 0.9044 | 0.9650 | 0.9985 | 4.014 | 2.336 | 0.177 |
| `para-carlito` | **0.2348** | 0.3495 | 0.6578 | **19.797** | 16.484 | 9.631 |

## Analysis

- `styles-tinos` and `para-gelasio` agree well: the sub-1.0 regions are the
  expected AA/hinting/subpixel noise plus small glyph-position deltas — the
  genuine cross-renderer noise floor.
- `para-carlito` diverges structurally. The failure heatmap shows text
  starting aligned at each line head and drifting rightward with cumulative
  error until the wrap points differ — the signature of **kerning**:
  LibreOffice applies Carlito's kern pairs, Loki's layout does not yet apply
  kerning (**fidelity gap #23**, `docs/audit-2026-06.md`). This is a *real,
  known fidelity gap that the calibration pass independently found and
  quantified*, not rasterizer noise — precisely the bug class the visual
  axis exists to catch.

## Decision

- **Calibrated thresholds** (derived from the two agreeing fixtures, with
  margin): `min_ssim = 0.60` (observed min on correct fixtures 0.6324),
  `max_delta_e = 10.0` (observed max 9.083). These pass the agreeing
  fixtures and fail `para-carlito`-class divergence.
- **The visual gate stays advisory** (not a hard CI gate) until fidelity
  gap #23 (kerning) is closed and this pass is re-run — hardening it now
  would permanently red a known, tracked gap. `visual_golden.rs` in
  `loki-render-cpu` asserts the two agreeing fixtures pass and pins
  `para-carlito`'s failure as an expected-divergence canary that flips when
  kerning lands.

## Reproduction

```sh
cp loki-fonts/fonts/*.ttf ~/.fonts && fc-cache
./scripts/generate-odf-goldens.sh
cargo run -p loki-render-cpu --example calibrate_odf
```
