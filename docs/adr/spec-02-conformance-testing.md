<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 02 — Conformance Testing Harness

| | |
|---|---|
| **Status** | Draft — pending implementation (B-1 resolved: `vello_cpu`) |
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

The harness is not greenfield. A working ACID harness already exists as **`loki-acid`** — it operationalizes a 141-case `TEST_PLAN.md` corpus (22 P0 / 82 P1 / 47 P2) with an SSIM differ, golden discovery, and glyph-coverage canaries. This spec's infrastructure is therefore framed as a **promotion of `loki-acid`'s reusable modules into a shared `appthere-conformance` crate** at the repo root (alongside the dylint/CI primitives from Spec 01), *not* a from-scratch build. Loki Text is its first consumer; the Presentation and Spreadsheet apps consume the same machinery through their own fixture corpora and model-equality implementations. This spec implements the Text suite and the shared crate; the other two apps' suites are out of scope here but must not require changes to the shared crate to exist.

This is **audit-first**: the inventory pass (already run) confirmed `loki-acid` as the foundation, `xmllint`/`soffice` as available, the metric-compatible fonts as bundled, and the gaps to close (no vendored schemas, no shared normalized-model differ, zero committed goldens, a hardcoded threshold, no in-process CPU rasterizer). References below are illustrative; inspect the live code before changing it.

---

## 2. Goals / Non-Goals

**Goals**

- A shared `appthere-conformance` crate, promoted from `loki-acid`, providing the golden harness, perceptual differ, schema validators, and round-trip helpers.
- Deterministic, reproducible visual comparison that runs **without a GPU** and in CI, via an in-process `vello_cpu` software rasterizer.
- Committed binary golden PNGs for reproducibility.
- A documented, repeatable golden-generation procedure for both reference apps.
- A calibrated, documented perceptual threshold that retires the hardcoded `SSIM_THRESHOLD = 0.98`.
- Schema-validation and round-trip CI gates plugged into the Spec 01 pipeline.

**Non-Goals**

- Presentation/Spreadsheet suites (separate consumers; this spec only must not block them).
- Testing the GPU render path's pixel output as the *fidelity* reference (see D2 — fidelity uses the CPU rasterizer; GPU/CPU parity is a separate, smaller, local concern).
- Authoring new fixtures beyond formalizing and filling gaps in the existing ~130-case corpus.
- Perfect cross-renderer pixel identity (impossible; the whole point of perceptual tolerance).

---

## 3. Working Method

1. **Promote, don't greenfield.** Lift `loki-acid`'s reusable modules (catalog, SSIM differ, golden discovery, glyph canaries) into `appthere-conformance` behind clean traits; map each of the 141 existing cases to one or more of the three axes (import-only, export-only, full round-trip).
2. **Fill the axis gaps** the inventory found: vendored schemas (Axis 1), a shared normalized-model differ (Axis 2), the `vello_cpu` candidate path + ΔE/worst-region scoring (Axis 3).
3. **Stand up generation** procedures and produce the first golden set.
4. **Calibrate** the perceptual threshold (§7.4), retiring the hardcoded `0.98`, and commit the calibration record.
5. **Wire CI**: schema + round-trip as hard gates; visual goldens as a gate once calibrated.

---

## 4. The Three Axes — boundaries

| Axis | Input | Compares | Runs in CI? | GPU? |
|------|-------|----------|-------------|------|
| Schema validation | Loki's exported file | XML against official schema | Yes (hard gate) | No |
| Round-trip stability | Fixture → Loki model | Normalized model equality | Yes (hard gate) | No |
| Visual goldens | Fixture → Loki `vello_cpu` render | Perceptual diff vs committed golden | Yes (once calibrated) | No (`vello_cpu` software rasterizer) |

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

Loki's production render path is Vello **0.6 on wgpu (GPU)**. GPU rasterization is not bit-reproducible across drivers and hardware, and the agent/CI environment has no GPU at all. (The audit confirmed `use_cpu: true` is only wgpu's *compute* fallback — it still requires a software adapter like llvmpipe, so it does not solve this.) Committing golden PNGs and comparing against a GPU render would be non-reproducible by construction.

**Decision (D2): conformance rendering uses `vello_cpu`, an in-process software rasterizer, at a fixed DPI and pinned anti-aliasing settings.** This makes the candidate side deterministic, reproducible, and runnable headless in CI and in the agent environment with no system graphics adapter at all. It is a genuinely *new* render path — Loki does not have one today — and the implementing agent must add a `vello_cpu` candidate entry point alongside the existing GPU path.

Carrying a second render path is the cost of this decision. Two things contain that cost: (1) the candidate path renders the same Vello scene graph the GPU path builds, so divergence is confined to the rasterizer, not the scene; and (2) the **CPU/GPU parity** check (the same scene rendered both ways, expected to agree within tolerance) exists precisely to keep the two honest. That parity check is local-only because it needs a GPU; fidelity-vs-reference always uses `vello_cpu`. Note that fidelity is judged against the *reference application's* golden, never against Loki's own GPU output, so `vello_cpu` not being pixel-identical to GPU Vello is expected and harmless.

### 7.2 Golden generation

Goldens are committed binary PNG blobs. Generation is offline (never in CI) and documented as a repeatable procedure:

- **OOXML goldens — Microsoft Office on Windows (manual).** Run on Kevin's Windows box. Provide a helper script (Word COM automation → export to PDF → rasterize each page with a pinned PDF rasterizer at the conformance DPI) so the human step is "run the script over the fixture set," and the rasterization stage is identical to every other path. The procedure is checked in; the operator and Office version are recorded next to the goldens.
- **ODF goldens — LibreOffice headless (scripted).** `soffice --headless` converting each fixture to PDF, then the same pinned PDF→PNG rasterizer at the conformance DPI. Scriptable end to end, which is why LO headless is the fast path for bulk initial generation.

Both reference apps rasterize *via PDF* through one shared rasterizer so the golden and candidate sides differ only in the layout/render engine, not in the PNG encoder or DPI.

### 7.3 Font parity

Comparison is meaningless unless both sides use the same fonts. Loki bundles the metric-compatible set: **Carlito** (≈ Calibri), **Caladea** (≈ Cambria), and the third bundled C-font equivalent; plus the newly-added **Tinos** (≈ Times New Roman), **Cousine** (≈ Courier New), **Arimo** (≈ Arial) for the classic web-safe faces. **Gelasio** (≈ Georgia) is added as a bundled asset under this spec — the audit found it the one missing metric-compatible face — so the corpus can exercise a Georgia-family fixture. The reference machines (Windows/Office, LibreOffice) must have the same set installed.

- **Fidelity fixtures reference the metric-compatible font names directly** (Carlito, Tinos, …). This isolates *rendering* from *substitution*: both reference app and Loki use the literal bundled font, so any diff is a rendering difference, not a substitution disagreement. The reference machines must have these fonts installed.
- **A separate, smaller substitution suite** authors fixtures referencing the *original* proprietary names (Calibri, Times New Roman) and asserts Loki's substitution engine maps them to the bundled equivalents and warns appropriately (this ties into the font-substitution warning redesigned in Spec 03). Kept apart so the substitution variable never contaminates fidelity scoring.

### 7.4 Perceptual diff & calibration

- **Metric:** structural similarity (SSIM) for layout/shape agreement, combined with a perceptual color delta (CIEDE2000 / ΔE in a perceptual space) for color/AA differences. Both are computed; a test fails if either crosses its threshold.
- **Regional scoring:** the page is tiled and scored per region, so a small localized failure (one mis-rendered glyph) isn't averaged away by a large correct page. The worst region drives the result.
- **Calibration, not a guessed number:** the harness today carries a hardcoded `SSIM_THRESHOLD = 0.98` — the same magic-number-as-folklore anti-pattern Spec 01's 1280px bug represents, and exactly what this decision retires. Run a calibration pass over a baseline set of fixtures believed correct, measure the *natural* cross-renderer noise floor (AA, hinting, subpixel positioning differ even when both are "right"), and set the default threshold a documented margin above that floor. The calibration record — corpus, measured distributions, chosen thresholds, date, font/tool versions — is committed alongside the goldens. The threshold is data, not folklore. (The Spec 01 `no_hardcoded_layout_dims` lint won't catch this literal — different code path — so the calibration requirement is the enforcement here.)
- **Per-test tolerance override** for legitimately hard cases (complex gradients, certain table borders), each override carrying a comment justifying it.
- **Failure artifacts:** on failure the harness emits a heatmap diff PNG and the per-region scores, so a regression is inspectable without re-running locally.

**Pass criterion:** every region under threshold (or under its justified per-test override). Becomes a CI gate once §7.4 calibration lands; advisory until then.

---

## 8. The Shared Crate — `appthere-conformance`

Created by **promoting `loki-acid`'s reusable modules** (catalog, SSIM differ, golden discovery, glyph canaries), not by greenfield authoring. Lives at the monorepo root with the Spec 01 enforcement primitives. Provides:

- `SchemaValidator` trait + libxml2-backed impl, with vendored pinned schemas. *(New — `loki-acid` has none.)*
- `RoundTrip` helpers + a normalized-model differ generic over a consumer-supplied model-equality impl. *(New shared layer over the 25 existing ad-hoc per-crate round-trip tests.)*
- `GoldenHarness`: fixture discovery (lifted from `loki-acid`), `vello_cpu` candidate render *(new)*, golden load, perceptual diff extended with ΔE + worst-region scoring + heatmaps *(SSIM lifted, the rest new)*, artifact emission.
- The pinned PDF→PNG rasterizer wrapper shared by generation and (where relevant) candidate paths.
- Calibration tooling. *(New — replaces the hardcoded threshold.)*

Consumers (Text, later Presentation/Spreadsheet) supply: a fixture corpus, a model type + normalized-equality impl, an importer/exporter pair, and a `vello_cpu` render entry point. They get all three axes for free. The crate must not contain Text-specific assumptions.

---

## 9. Fixture Corpus

Formalize the existing **141-case** corpus (the `loki-acid` `TEST_PLAN.md`: 22 P0 / 82 P1 / 47 P2) into a discoverable on-disk layout, organized by **feature × format × axis**, retaining the P0–P2 severity tags. Each fixture records: the feature it exercises, which axes apply, the reference app and version used for its golden, and any per-test tolerance override with justification. The known gap (PPTX generation incomplete due to a timeout) is Presentation-suite scope, not Text; note it as a cross-reference, don't solve it here.

---

## 10. Key Decisions (ADR-style)

**D1 — Three independent axes.** Schema, round-trip, and visual fail for different reasons and catch different bugs; coupling them would mask faults. Tradeoff: more infrastructure than a single "does it look right" check, justified by diagnosability.

**D2 — Conformance renders on `vello_cpu` (in-process software rasterizer), not GPU.** Determinism and CI/agent-without-GPU demand it; committed goldens require a reproducible candidate, and wgpu's `use_cpu` compute fallback still needs a software adapter so it doesn't qualify. GPU correctness is covered by a separate local CPU/GPU parity check. Tradeoff: a genuinely new second render path to maintain — accepted because it shares the scene graph with the GPU path and the parity check keeps them aligned, and because the alternative (pinned-llvmpipe wgpu or a local-only visual gate) either adds a brittle pinned system dependency or removes CI visual coverage entirely.

**D3 — Both reference apps rasterize via one shared PDF→PNG stage.** Golden and candidate then differ only in layout/render engine, not encoder/DPI. Tradeoff: a PDF round-trip on the golden side; acceptable since it's identical across all goldens.

**D4 — Fidelity fixtures use metric-compatible font names directly; substitution gets its own suite.** Isolates rendering from substitution so a diff has one meaning. Tradeoff: real-world docs reference proprietary names — covered by the dedicated substitution suite instead.

**D5 — Threshold is calibrated and committed, not guessed.** A documented noise-floor measurement sets it; the record lives in the repo. Tradeoff: a calibration step before the visual gate can turn hard — worth it to avoid a meaningless magic number.

**D6 — Schemas and tools are vendored and version-pinned.** Reproducible, offline validation. Tradeoff: periodic manual schema updates; acceptable for determinism.

---

## 11. Milestones & Acceptance Criteria

**M1 — Promote `loki-acid` to the shared crate skeleton.** `appthere-conformance` with the three axis traits and the rasterizer wrapper, lifting `loki-acid`'s catalog/SSIM/discovery; Text wired as the first consumer; the 141 cases mapped to axes. *Accept:* a trivial fixture flows through all three axes (even if visual is advisory); no regression in existing `loki-acid` coverage.

**M2 — Schema axis live.** Vendored pinned OOXML/ODF schemas; OPC + manifest validation; libxml2-backed validator (`xmllint` confirmed available). *Accept:* a deliberately malformed export fails the gate; valid exports pass; missing `xmllint` fails loudly.

**M3 — Round-trip axis live.** Shared normalized differ unifying the 25 existing ad-hoc round-trip tests + all three round-trip shapes; first-divergence reporting. *Accept:* a seeded loss (e.g. a dropped run property) is caught with a model path; the bookmark-id class is covered.

**M4 — Golden generation.** Documented OOXML (manual/COM) and ODF (LO headless — `soffice` confirmed available) procedures; shared PDF→PNG rasterizer; first golden set committed with operator/version metadata. *Accept:* regenerating an ODF golden from scratch reproduces the committed bytes; the OOXML procedure is runnable by following the checked-in doc.

**M5 — `vello_cpu` candidate path + visual axis + calibration.** New `vello_cpu` render entry point; SSIM + ΔE regional differ replacing the hardcoded `0.98`; committed calibration record; failure heatmaps. *Accept:* the `vello_cpu` path renders headless with no graphics adapter; a deliberately mis-rendered fixture fails with a heatmap; a correct one passes; the threshold traces to the calibration data, not a literal.

**M6 — CI integration.** Schema + round-trip as hard gates; visual as a gate post-calibration. *Accept:* the full suite runs headless in CI with no GPU; the three gates can fail a PR independently.

---

## 12. Out of Scope

- Presentation and Spreadsheet suites (separate consumers; the shared crate must merely not block them). The incomplete PPTX generation is theirs.
- GPU render-path pixel testing as the fidelity reference (→ local CPU/GPU parity check only). Pinned-llvmpipe wgpu and a local-only visual gate were considered and rejected in favor of `vello_cpu` (see D2).
- Authoring net-new fixtures beyond filling corpus gaps.
- The font-substitution *warning UI* (→ Spec 03); this spec only exercises the substitution *engine* in its dedicated suite.
- A pure-Rust schema validator (the trait permits one later; the first impl shells to libxml2).
