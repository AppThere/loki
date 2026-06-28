<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 02 — Conformance Testing Harness

| | |
|---|---|
| **Status** | Draft — pending implementation |
| **Scope** | AppThere Loki (Text), DOCX + ODT; harness designed as shared monorepo infrastructure |
| **Sequence** | 2 of 6 — establishes model trustworthiness before styling is built on top |
| **Depends on** | Spec 01 (CI pipeline, the `Viewport`/`LayoutContext` type, enforcement primitives at monorepo root) |
| **Feeds** | Styling Panel (trusted model), and the schema/round-trip CI gates Spec 01 reserved a slot for |

---

## 1. Context & Motivation

Loki claims rendering and round-trip fidelity against two reference implementations: **Microsoft Office** (the OOXML gold standard) and **LibreOffice** (the ODF gold standard). Today that claim is asserted by an ad-hoc ACID test plan (~130 cases, P0–P2) but not mechanically verified end to end. This spec builds the harness that verifies it, on three independent axes:

1. **Schema validation** — exported files are well-formed against the official OOXML (XSD / ISO-IEC 29500) and ODF (RELAX NG / OASIS) schemas. Catches malformed output regardless of how it renders.
2. **Round-trip stability** — importing, exporting, and re-importing a document does not silently lose or mutate semantic content. Compared on the *model*, not the bytes.
3. **Visual goldens** — Loki's render of a fixture matches a committed golden PNG produced by the reference application, within a calibrated perceptual tolerance.

These axes are independent on purpose: a file can be schema-valid but render wrong, or render right but lose data on re-save. Each axis fails for its own reason and points at its own bug class.

The harness itself is **shared monorepo infrastructure** (proposed crate `appthere-conformance` at the repo root, alongside the dylint/CI primitives from Spec 01). Loki Text is its first consumer; the Presentation and Spreadsheet apps consume the same machinery through their own fixture corpora and model-equality implementations. This spec implements the Text suite and the shared crate; the other two apps' suites are out of scope here but must not require changes to the shared crate to exist.

This is **audit-first**: inspect the existing ACID cases, the import/export crates, and the render path before building. References below are illustrative.

---

## 2. Goals / Non-Goals

**Goals**

- A shared `appthere-conformance` crate providing the golden harness, perceptual differ, schema validators, and round-trip helpers.
- Deterministic, reproducible visual comparison that runs **without a GPU** and in CI.
- Committed binary golden PNGs for reproducibility.
- A documented, repeatable golden-generation procedure for both reference apps.
- A calibrated, documented perceptual threshold rather than a guessed magic number.
- Schema-validation and round-trip CI gates plugged into the Spec 01 pipeline.

**Non-Goals**

- Presentation/Spreadsheet suites (separate consumers; this spec only must not block them).
- Testing the GPU render path's pixel output as the *fidelity* reference (see D2 — fidelity uses the CPU rasterizer; GPU/CPU parity is a separate, smaller, local concern).
- Authoring new fixtures beyond formalizing and filling gaps in the existing ~130-case corpus.
- Perfect cross-renderer pixel identity (impossible; the whole point of perceptual tolerance).

---

## 3. Working Method

1. **Inventory** the existing ACID corpus and map each case to one or more of the three axes. Identify cases that exercise only import, only export, or full round-trip.
2. **Build the shared crate** with the three axis modules behind clean traits, so a consuming app supplies fixtures + a model-equality impl and gets all three axes.
3. **Stand up generation** procedures and produce the first golden set.
4. **Calibrate** the perceptual threshold (§7.4) and commit the calibration record.
5. **Wire CI**: schema + round-trip as hard gates; visual goldens as a gate once calibrated.

---

## 4. The Three Axes — boundaries

| Axis | Input | Compares | Runs in CI? | GPU? |
|------|-------|----------|-------------|------|
| Schema validation | Loki's exported file | XML against official schema | Yes (hard gate) | No |
| Round-trip stability | Fixture → Loki model | Normalized model equality | Yes (hard gate) | No |
| Visual goldens | Fixture → Loki CPU render | Perceptual diff vs committed golden | Yes (once calibrated) | No (CPU rasterizer) |

All three are headless and GPU-free, which is what lets them run in CI and inside the GPU-less agent environment.

---

## 5. Axis 1 — Schema Validation

For every export the harness produces, validate the serialized XML against the official schema before any rendering question is asked.

- **OOXML**: validate each significant part (`document.xml`, `styles.xml`, `numbering.xml`, …) against the ISO/IEC 29500 schemas. Validate **Transitional** by default, with Strict as an opt-in stricter pass. Additionally validate the OPC layer: `[Content_Types].xml`, relationship parts, and package structure (this is where `loki-opc` correctness is exercised).
- **ODF**: validate against the OASIS ODF RELAX NG schema, plus the ODF manifest.

**Implementation note:** a pure-Rust XSD/RNG validator is scarce. The pragmatic path is shelling to `libxml2` (`xmllint --schema` for XSD, `xmllint --relaxng` for RNG), with the schema files vendored and version-pinned in the repo so validation is reproducible and offline. The shared crate wraps this behind a `SchemaValidator` trait so a future pure-Rust backend can replace it without touching consumers. Validator availability is a build-time check, not a silent skip — a missing `xmllint` fails loudly.

**Pass criterion:** zero schema violations. This is a hard CI gate (the slot Spec 01 reserved).

---

## 6. Axis 2 — Round-Trip Stability

Three round-trip shapes, all compared on a **normalized model**, never on bytes (byte equality is neither achievable nor meaningful):

1. **Native**: `model → serialize → deserialize → model'`. Asserts the serializer/deserializer pair is lossless for Loki's own model.
2. **Import-export-import**: `fixture → import → export → import → model'`, comparing the two imported models. Asserts export doesn't drop what import understood.
3. **Reference-anchored**: import a reference-app-authored file, export it, re-import, assert stability. Catches asymmetries between what Loki reads and what it writes.

**Normalized equality.** Define a canonical form that ignores semantically-insignificant differences (element ordering where the format permits, whitespace normalization, default-value elision) but catches real loss (a dropped run property, a collapsed style, a mangled bookmark id — cf. the Fix Session 01 bookmark bug). On mismatch, the differ reports the **first divergence with a model path**, not just a boolean, so failures are diagnosable.

**Pass criterion:** normalized model equality across all three shapes. Hard CI gate.

---

## 7. Axis 3 — Visual Goldens

The involved axis. A fixture document is rendered by the reference app (the golden) and by Loki (the candidate), and the two PNGs are compared perceptually.

### 7.1 Determinism is the whole problem

Loki's production render path is Vello on **wgpu (GPU)**. GPU rasterization is not bit-reproducible across drivers and hardware, and the agent/CI environment has no GPU at all. Committing golden PNGs and comparing against a GPU render would be non-reproducible by construction.

**Decision (D2): conformance rendering uses Vello's CPU rasterizer path, at a fixed DPI and pinned anti-aliasing settings.** This makes the candidate side deterministic, reproducible, and runnable headless in CI and in the agent environment. The GPU path's correctness is covered separately by a smaller **CPU/GPU parity** check (the same scene rendered both ways agrees within tolerance), which is local-only because it needs a GPU. Fidelity-vs-reference always uses CPU.

### 7.2 Golden generation

Goldens are committed binary PNG blobs. Generation is offline (never in CI) and documented as a repeatable procedure:

- **OOXML goldens — Microsoft Office on Windows (manual).** Run on Kevin's Windows box. Provide a helper script (Word COM automation → export to PDF → rasterize each page with a pinned PDF rasterizer at the conformance DPI) so the human step is "run the script over the fixture set," and the rasterization stage is identical to every other path. The procedure is checked in; the operator and Office version are recorded next to the goldens.
- **ODF goldens — LibreOffice headless (scripted).** `soffice --headless` converting each fixture to PDF, then the same pinned PDF→PNG rasterizer at the conformance DPI. Scriptable end to end, which is why LO headless is the fast path for bulk initial generation.

Both reference apps rasterize *via PDF* through one shared rasterizer so the golden and candidate sides differ only in the layout/render engine, not in the PNG encoder or DPI.

### 7.3 Font parity

Comparison is meaningless unless both sides use the same fonts. Loki bundles the metric-compatible set: **Carlito** (≈ Calibri), **Caladea** (≈ Cambria), and the third bundled C-font equivalent; plus the newly-added **Tinos** (≈ Times New Roman), **Cousine** (≈ Courier New), **Arimo** (≈ Arial) for the classic web-safe faces.

- **Fidelity fixtures reference the metric-compatible font names directly** (Carlito, Tinos, …). This isolates *rendering* from *substitution*: both reference app and Loki use the literal bundled font, so any diff is a rendering difference, not a substitution disagreement. The reference machines must have these fonts installed.
- **A separate, smaller substitution suite** authors fixtures referencing the *original* proprietary names (Calibri, Times New Roman) and asserts Loki's substitution engine maps them to the bundled equivalents and warns appropriately (this ties into the font-substitution warning redesigned in Spec 03). Kept apart so the substitution variable never contaminates fidelity scoring.

### 7.4 Perceptual diff & calibration

- **Metric:** structural similarity (SSIM) for layout/shape agreement, combined with a perceptual color delta (CIEDE2000 / ΔE in a perceptual space) for color/AA differences. Both are computed; a test fails if either crosses its threshold.
- **Regional scoring:** the page is tiled and scored per region, so a small localized failure (one mis-rendered glyph) isn't averaged away by a large correct page. The worst region drives the result.
- **Calibration, not a guessed number:** run a calibration pass over a baseline set of fixtures believed correct, measure the *natural* cross-renderer noise floor (AA, hinting, subpixel positioning differ even when both are "right"), and set the default threshold a documented margin above that floor. The calibration record — corpus, measured distributions, chosen thresholds, date, font/tool versions — is committed alongside the goldens. The threshold is data, not folklore.
- **Per-test tolerance override** for legitimately hard cases (complex gradients, certain table borders), each override carrying a comment justifying it.
- **Failure artifacts:** on failure the harness emits a heatmap diff PNG and the per-region scores, so a regression is inspectable without re-running locally.

**Pass criterion:** every region under threshold (or under its justified per-test override). Becomes a CI gate once §7.4 calibration lands; advisory until then.

---

## 8. The Shared Crate — `appthere-conformance`

Lives at the monorepo root with the Spec 01 enforcement primitives. Provides:

- `SchemaValidator` trait + libxml2-backed impl, with vendored pinned schemas.
- `RoundTrip` helpers + a normalized-model differ generic over a consumer-supplied model-equality impl.
- `GoldenHarness`: fixture discovery, candidate CPU render, golden load, perceptual diff, artifact emission.
- The pinned PDF→PNG rasterizer wrapper shared by generation and (where relevant) candidate paths.
- Calibration tooling.

Consumers (Text, later Presentation/Spreadsheet) supply: a fixture corpus, a model type + normalized-equality impl, an importer/exporter pair, and a CPU render entry point. They get all three axes for free. The crate must not contain Text-specific assumptions.

---

## 9. Fixture Corpus

Formalize the existing ~130 ACID cases into a discoverable on-disk layout, organized by **feature × format × axis**, retaining the P0–P2 severity tags. Each fixture records: the feature it exercises, which axes apply, the reference app and version used for its golden, and any per-test tolerance override with justification. The known gap (PPTX generation incomplete due to a timeout) is Presentation-suite scope, not Text; note it as a cross-reference, don't solve it here.

---

## 10. Key Decisions (ADR-style)

**D1 — Three independent axes.** Schema, round-trip, and visual fail for different reasons and catch different bugs; coupling them would mask faults. Tradeoff: more infrastructure than a single "does it look right" check, justified by diagnosability.

**D2 — Conformance renders on the CPU rasterizer, not GPU.** Determinism and CI/agent-without-GPU demand it; committed goldens require a reproducible candidate. GPU correctness is covered by a separate local CPU/GPU parity check. Tradeoff: the fidelity suite doesn't exercise the production GPU path directly — accepted, because the alternative is non-reproducible goldens.

**D3 — Both reference apps rasterize via one shared PDF→PNG stage.** Golden and candidate then differ only in layout/render engine, not encoder/DPI. Tradeoff: a PDF round-trip on the golden side; acceptable since it's identical across all goldens.

**D4 — Fidelity fixtures use metric-compatible font names directly; substitution gets its own suite.** Isolates rendering from substitution so a diff has one meaning. Tradeoff: real-world docs reference proprietary names — covered by the dedicated substitution suite instead.

**D5 — Threshold is calibrated and committed, not guessed.** A documented noise-floor measurement sets it; the record lives in the repo. Tradeoff: a calibration step before the visual gate can turn hard — worth it to avoid a meaningless magic number.

**D6 — Schemas and tools are vendored and version-pinned.** Reproducible, offline validation. Tradeoff: periodic manual schema updates; acceptable for determinism.

---

## 11. Milestones & Acceptance Criteria

**M1 — Shared crate skeleton.** `appthere-conformance` with the three axis traits and the rasterizer wrapper; Text wired as the first consumer. *Accept:* a trivial fixture flows through all three axes (even if visual is advisory).

**M2 — Schema axis live.** Vendored pinned OOXML/ODF schemas; OPC + manifest validation; libxml2-backed validator. *Accept:* a deliberately malformed export fails the gate; valid exports pass; missing `xmllint` fails loudly.

**M3 — Round-trip axis live.** Normalized differ + all three round-trip shapes; first-divergence reporting. *Accept:* a seeded loss (e.g. a dropped run property) is caught with a model path; the bookmark-id class is covered.

**M4 — Golden generation.** Documented OOXML (manual/COM) and ODF (LO headless) procedures; shared PDF→PNG rasterizer; first golden set committed with operator/version metadata. *Accept:* regenerating an ODF golden from scratch reproduces the committed bytes; the OOXML procedure is runnable by following the checked-in doc.

**M5 — Visual axis + calibration.** CPU-rasterizer candidate path; SSIM + ΔE regional differ; committed calibration record; failure heatmaps. *Accept:* a deliberately mis-rendered fixture fails with a heatmap; a correct one passes; the threshold traces to the calibration data.

**M6 — CI integration.** Schema + round-trip as hard gates; visual as a gate post-calibration. *Accept:* the full suite runs headless in CI with no GPU; the three gates can fail a PR independently.

---

## 12. Out of Scope

- Presentation and Spreadsheet suites (separate consumers; the shared crate must merely not block them). The incomplete PPTX generation is theirs.
- GPU render-path pixel testing as the fidelity reference (→ local CPU/GPU parity check only).
- Authoring net-new fixtures beyond filling corpus gaps.
- The font-substitution *warning UI* (→ Spec 03); this spec only exercises the substitution *engine* in its dedicated suite.
- A pure-Rust schema validator (the trait permits one later; the first impl shells to libxml2).
