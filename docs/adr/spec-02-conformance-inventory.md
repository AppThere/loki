<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 02 — Conformance Inventory & Readiness (audit-first)

| | |
|---|---|
| **Status** | Draft — inventory complete; B-1 + B-10 resolved; B-11 unblocked; **build progressing: M1 skeleton + M2 schema axis landed; M3 round-trip axis largely done — differ + deepened canonicalizer (tables/metadata) + all 3 shapes (DOCX/ODT/XLSX) wired, 2 export bugs found & fixed (B-7); B-6/B-8 in progress** |
| **Companion to** | [`spec-02-conformance-testing.md`](spec-02-conformance-testing.md) |
| **Method** | Read-only audit pass (§3.1). **No code changed.** |
| **Snapshot** | branch `claude/adr-docs-setup-ogwz5a`, 2026-06-28 |
| **Depends on** | Spec 01 ([`spec-01-audit-report.md`](spec-01-audit-report.md), [`0009-target-architecture.md`](0009-target-architecture.md)) |

> **Resolution update (2026-06-28).** After this inventory was filed, the maintainer
> resolved the two findings that needed a decision, and the spec was revised
> ([`spec-02-conformance-testing.md`](spec-02-conformance-testing.md), D2 / §7.3):
> - **B-1 — CPU rasterizer:** resolved in favour of an **in-process `vello_cpu`
>   software rasterizer** (a new conformance-only render path). The llvmpipe-backed
>   wgpu alternative was rejected. No longer an open blocker.
> - **B-10 — Gelasio (≈Georgia):** resolved — **Gelasio is to be bundled** under
>   Spec 02, completing the metric-compatible font set.
>
> The original findings are preserved below as the audit-first snapshot; each is
> annotated inline with its resolution. All other findings (B-2…B-9) remain
> open for triage — the **Priority** column is still the maintainer's.

> **Spec 01 enforcement update (2026-06-29).** When this inventory was filed,
> Spec 01's CI pipeline was *"specified but not yet implemented"*, and **B-11**
> (and §8) flagged that M6 (CI integration) was blocked on it. **That work has
> since landed:** `.github/workflows/rust.yml`'s lint job now runs **nine
> structural gates** (license-header, panic-guard, unsafe-policy, file-ceiling,
> TODO-format, `unwrap_used`/`expect_used`, dependency-direction, viewport-dim) —
> see [`spec-01-audit-report.md`](spec-01-audit-report.md) §4. So **B-11 is no
> longer blocked**: the conformance schema + round-trip gates plug into the
> existing, populated pipeline using the same script-gate-→-lint-job pattern. The
> few other Spec-01-dependent notes below (B-6's loki-opc SPDX reference, B-3's
> "1280px-class" analogy) are annotated where they occur.

Spec 02 §1/§3 mandate **audit-first**: "inspect the existing ACID cases, the
import/export crates, and the render path before building." This document is that
inventory — it maps the existing corpus to the three axes, assesses what
machinery already exists vs. must be built, and surfaced the blocking decisions
(chiefly the CPU-rasterizer gap, now resolved per the banner) so the maintainer
could triage before any of M1–M6 is implemented. Mirrors the Spec 01 deliverable
shape; **no crate or gate is built in this pass.**

---

## 1. Executive summary

Loki already has a real fidelity harness — **`loki-acid`** — that operationalises
a **141-case** `TEST_PLAN.md` corpus and ships much of what Spec 02 calls for
(machine-readable catalog, SSIM differ, golden discovery, glyph-coverage
canaries). The shared `appthere-conformance` crate the spec proposes **does not
exist yet**; the pragmatic path is to *promote and generalise `loki-acid`'s
modules* into it rather than greenfield.

Axis-by-axis readiness:

| Axis | Existing today | Gap to Spec 02 |
|---|---|---|
| **1 — Schema** | nothing; **0 vendored schemas** | Whole axis. *But* `xmllint` **is installed** here, so D6 is feasible immediately. |
| **2 — Round-trip** | **25 ad-hoc round-trip test files** across `loki-odf`/`loki-ooxml`/etc. | No *normalized-model* differ, no first-divergence path reporting, no unified 3-shape harness. |
| **3 — Visual goldens** | SSIM differ + golden discovery + a `golden_pixel` test | **0 goldens committed**; no in-process CPU rasterizer (**B-1 resolved → build `vello_cpu`**); threshold is a **guessed `0.98`** (violates D5); SSIM-only (no ΔE, no worst-region). |

**The single blocking finding (B-1) — now resolved.** Loki renders through `vello
= "0.6"` on **wgpu (GPU)**; there is **no `vello_cpu` dependency** and **no
software adapter visible** in this environment, so D2's CPU-rasterizer path was
unbuilt and was the critical-path decision for the visual axis. **Maintainer
resolution: adopt an in-process `vello_cpu` software rasterizer** (revised spec
D2). The finding is now an implementation task (M5), not an open question;
everything else was already reachable.

Good news that de-risks the rest: **`xmllint` and `soffice`/`libreoffice` are
both present**, and the full metric-compatible font set (Carlito, Caladea, Tinos,
Cousine, Arimo) is already bundled.

---

## 2. The existing harness — `loki-acid` (map to the shared crate)

`loki-acid` is structured almost exactly along the shared crate's responsibility
lines (§8). Promotion/reuse map:

| `appthere-conformance` need (§8) | `loki-acid` today | Action |
|---|---|---|
| `GoldenHarness` (discovery, diff, artifacts) | `golden.rs` (discovery) + `diff.rs` (SSIM) + `tests/golden_pixel.rs` | **Promote & generalise** — strip Text-specific assumptions; add ΔE + regional scoring + heatmap emit |
| Perceptual differ | `diff.rs`: `mean_ssim`, `window_ssim`, `abs_diff_ratio` | **Reuse SSIM**; add CIEDE2000/ΔE + worst-region selection |
| Fixture corpus | `catalog/` (141 `TC-*`), `fixtures.rs` (`include_bytes!`), `assets/` | **Reuse & reorganise** to feature×format×axis (§9) |
| Severity tags | `severity.rs` (`P0`/`P1`/`P2`) | **Reuse as-is** |
| Glyph-coverage / page-count canaries | `pages.rs` | **Reuse** (cheap pre-pixel gate, GPU-free) |
| `SchemaValidator` trait + libxml2 impl | **none** | **Build** (Axis 1) |
| `RoundTrip` + normalized-model differ | per-crate ad-hoc tests | **Build** the unified differ; fold existing tests in |
| Pinned PDF→PNG rasterizer wrapper | **none** | **Build** (shared by golden gen + reference) |
| Calibration tooling | **none**; threshold hardcoded | **Build** (§7.4) |

Constraint from §8: the shared crate "must not contain Text-specific
assumptions." `loki-acid` today hardcodes the fixture enum and Text/Sheet
pipelines (`fixtures.rs`), so promotion requires extracting a `Fixture`/`Consumer`
trait — flagged as the main refactor cost.

---

## 3. Axis 1 — Schema validation (status: unbuilt, but unblocked)

- **Vendored schemas: none.** `git ls-files` finds **0** `.xsd`/`.rng` files.
  (`loki-doc-model/src/loro_schema.rs` is the Loro CRDT schema — unrelated.) The
  ISO/IEC 29500 (OOXML Transitional + Strict) XSDs and the OASIS ODF RELAX NG
  must be vendored and version-pinned (D6).
- **Validator backend: available now.** `xmllint` is at `/usr/bin/xmllint`, so
  the `SchemaValidator` libxml2 impl (`--schema` for XSD, `--relaxng` for RNG) is
  immediately runnable, and the "missing `xmllint` fails loudly" build-time check
  (§5) is straightforward.
- **OPC layer:** `loki-opc` produces `[Content_Types].xml` + relationship parts —
  the crate Axis 1's package-structure validation exercises. (The Spec 01 A-3
  SPDX-header finding once noted here is now resolved: `loki-opc` is MIT-licensed
  with correct headers — see [`0010-per-crate-licensing.md`](0010-per-crate-licensing.md).)
- **Export surfaces to validate** (from the crate graph): DOCX export
  (`loki-ooxml/src/docx/write/`), ODT/ODS export (`loki-odf/src/odt/write/`,
  `ods/export.rs`), XLSX export (`loki-ooxml/src/xlsx/export.rs`). Each emits the
  parts the schema axis must check.

**Verdict:** entirely build-work, but no environmental blocker. Lowest-risk axis
to land first (matches M2).

---

## 4. Axis 2 — Round-trip (status: rich but un-unified)

There is already **substantial** round-trip testing — **25 files**:

- `loki-odf`: `round_trip.rs`, `round_trip_odf{,2,3,4,5}.rs`, `roundtrip.rs`,
  `ods_round_trip.rs`, `odt_export_round_trip.rs`, `math_round_trip.rs` (11).
- `loki-ooxml`: `round_trip.rs`, `round_trip_conformance{,2,3,4,5}.rs`,
  `comments_round_trip.rs`, `metadata_round_trip.rs`, `style_round_trip.rs`,
  `math_round_trip.rs`, `pptx_round_trip.rs`, `xlsx_round_trip.rs` (≈14).
- `loki-doc-model` (`loro_styles_round_trip.rs`), `loki-sheet-model`,
  `loki-templates`.

What's **missing** vs §6:

- These compare per-crate, often on serialized output or bespoke assertions —
  there is **no shared *normalized-model* canonical form** with default-value
  elision / order-insensitivity, and **no first-divergence-with-model-path**
  reporting. On failure they give a boolean/`assert_eq`, not a diagnosable path.
- The three **shapes** (§6) aren't cleanly separated. Mapping what exists:
  - *Native* (`model→serialize→deserialize→model'`): partially covered by
    `loro_styles_round_trip` / export round-trips.
  - *Import-export-import*: closest to `round_trip_conformance*` in `loki-ooxml`.
  - *Reference-anchored*: the `acid_*.{docx,odt}` fixtures (authored by
    Office/LO) exist in `loki-acid/assets/`, but no test does import→export→
    re-import stability on them.
- The "bookmark-id class" the spec calls out (§6 / M3) needs a dedicated
  normalized-equality case; not currently isolated.

**Verdict:** the *coverage* exists; the *infrastructure* (one normalized differ,
generic over a consumer model-equality impl, with path-precise diffs) does not.
Fold the 25 existing tests into it rather than discard.

---

## 5. Axis 3 — Visual goldens (status: scaffold only; one hard blocker)

### 5.1 What exists

- `diff.rs`: windowed SSIM (`mean_ssim` tiles into `WINDOW`-sized blocks but
  **averages** them), plus `abs_diff_ratio`. Pure-Rust, GPU-free. ✅
- `golden.rs`: `goldens/<stem>/page-NNN.png` ↔ `renders/<stem>/page-NNN.png`
  discovery. ✅
- `tests/golden_pixel.rs`: SSIM-asserts matched pairs.

### 5.2 Gaps (each a triage item)

- **B-1 — No CPU rasterizer (was blocking, D2). ✅ RESOLVED → `vello_cpu`.**
  Render path is `vello = "0.6"` on `wgpu = "26"`. `loki-renderer/src/vello_init.rs`
  sets `use_cpu: true`, but that is wgpu's CPU *compute* fallback — it still
  requires a wgpu adapter (the `render_to_png` example aborts with "a software
  rasterizer e.g. llvmpipe is required"), and no software adapter is visible in
  this environment. There is **no `vello_cpu` crate** in the tree. The golden
  harness consumes pre-rendered PNGs from `renders/` — i.e. rendering is
  **external today**, exactly the non-reproducibility D2 wants to eliminate.
  **Maintainer decision:** adopt **`vello_cpu`** (the Vello project's CPU
  rasterizer) as a conformance-only render backend; llvmpipe-backed wgpu rejected
  (revised spec D2 / §12). Now an M5 implementation task — add the `vello_cpu`
  candidate entry point alongside the GPU path.
- **B-2 — Zero goldens committed.** `loki-acid/goldens/` holds only `README.md`;
  `renders/` is empty; `find … -name '*.png'` = 0. The visual axis has **no
  reference data**. M4 (generation) must run before M5 means anything. `soffice`
  is present → ODF bulk generation (LO headless → PDF → PNG) is runnable here;
  OOXML goldens still need the manual Windows/Office COM procedure (§7.2).
- **B-3 — Threshold is a guessed magic number (violates D5).**
  `golden_pixel.rs:18` hardcodes `const SSIM_THRESHOLD: f64 = 0.98;` with no
  calibration record. D5/§7.4 require this be *derived* from a measured
  cross-renderer noise floor and committed. (This is precisely the "1280px-class"
  magic-number pattern Spec 01 just **eliminated** in the editor — A-1 — applied
  here in the test domain: a load-bearing literal nobody measured.)
- **B-4 — Differ is SSIM-only and averaged.** §7.4 wants **SSIM + CIEDE2000/ΔE**,
  **regional/tiled scoring where the worst region drives the result** (not the
  mean), per-test tolerance overrides, and **failure heatmaps**. Today: mean SSIM,
  no ΔE, no worst-region, no heatmap, no per-test override.
- **B-5 — No shared PDF→PNG rasterizer (D3).** Both reference apps must rasterize
  via one pinned PDF→PNG stage; that wrapper doesn't exist. `loki-pdf` exists for
  *export*, and `loki-acid/examples/render_acid_pdf.rs` exists, but no pinned
  PDF→raster stage is shared between golden and candidate.

### 5.3 Fonts (D4) — essentially satisfied

All metric-compatible faces are **already bundled** in `loki-fonts/fonts/`:
Carlito (≈Calibri), Caladea (≈Cambria), Tinos (≈Times New Roman), Cousine
(≈Courier New), Arimo (≈Arial), each in Regular/Bold/Italic/BoldItalic, plus
AtkinsonHyperlegibleNext. `loki-fonts/src/lib.rs` maps the proprietary names
(Calibri/Cambria) to substitutes. So:

- **Fidelity fixtures** (reference the bundled names directly) — ready.
- **Substitution suite** (reference Calibri/Times New Roman, assert mapping +
  warning) — the engine exists (`lib.rs` mappings); the dedicated suite does not.
- **Caveat (B-10) — ✅ RESOLVED.** The spec's "third bundled C-font equivalent"
  (Gelasio ≈ Georgia) was **not present** — only Carlito + Caladea of that family.
  **Maintainer decision: bundle Gelasio** under Spec 02 (revised spec §7.3), so the
  corpus can exercise a Georgia-family fixture. Now an asset-add task.
- **Reference machines must install these fonts** for golden generation to be
  meaningful (operator note for §7.2).

---

## 6. Environment capability check (what this box can/can't do)

| Capability | Needed by | Status here |
|---|---|---|
| `xmllint` (libxml2) | Axis 1, M2 | ✅ `/usr/bin/xmllint` |
| `soffice` / `libreoffice` headless | M4 ODF goldens | ✅ `/usr/bin/soffice` |
| MS Office / Word COM | M4 OOXML goldens | ❌ Windows-only (Kevin's box; manual) |
| wgpu software adapter (llvmpipe) | GPU render / CPU-compute fallback | ❌ none visible — rejected alternative (see B-1) |
| `vello_cpu` rasterizer | D2 candidate render | ❌ not yet a dependency — **to be added (B-1 resolved)** |
| Metric-compatible fonts | D4 | ✅ bundled in `loki-fonts` (Gelasio to be added, B-10) |

The agent/CI environment is **GPU-free**, exactly the condition §4/§7.1 target —
which is *why* the resolved B-1 choice (an in-process `vello_cpu` rasterizer,
needing no graphics adapter) is what lets the visual axis run here at all.

---

## 7. Triage table — Priority left blank for maintainer (mirrors Spec 01 D1)

| ID | Axis/Area | Finding | Blast radius | Proposed action | Priority |
|----|-----------|---------|--------------|-----------------|----------|
| B-1 | Visual / D2 | No CPU rasterizer; render is GPU-only, external PNGs | **Foundational** | ✅ **Resolved → `vello_cpu`** (llvmpipe-wgpu rejected); build conformance-only `vello_cpu` render entry point (M5) | **Resolved** |
| B-2 | Visual / M4 | 0 goldens committed | **Large** | Stand up generation; LO-headless ODF set first (runnable here), OOXML manual | |
| B-3 | Visual / D5 | Threshold hardcoded `0.98`, uncalibrated | **Small (now), gating (later)** | Calibration pass + committed record; replace constant | |
| B-4 | Visual / §7.4 | SSIM-only, averaged; no ΔE/worst-region/heatmap/override | **Medium** | Extend `diff.rs`: CIEDE2000, regional worst-region, heatmap, per-test override | |
| B-5 | Visual / D3 | No shared pinned PDF→PNG stage | **Medium** | Build rasterizer wrapper; share golden+candidate | |
| B-6 | Schema / M2 | 0 vendored schemas | **Medium** | 🔨 **Validator done** — `schema::SchemaValidator` + libxml2 `XmllintValidator` (XSD + RNG, located violations, fails-loud), tested; CI installs `libxml2-utils`. **Remaining:** vendor + pin ISO 29500 / ODF RNG and validate real exports (`schemas/README.md`). | |
| B-7 | Round-trip / M3 | 25 ad-hoc tests, no normalized differ / path reporting | **Medium** | 🔨 **Differ + model adapter + first shape wired** — `roundtrip::{NormalizedModel, first_divergence}` plus `model::canonicalize_document` (feature `doc-model`): `NormalizedModel for loki_doc_model::Document` walking paragraphs/runs/marks/bookmarks/styles. Tested on real `Document`s incl. dropped run property, changed property, **mangled bookmark id**, dropped text, structural change. **Import→export→import shape now live** in `loki-ooxml` (dev-dep `appthere-conformance/doc-model`): `conformance_round_trip.rs` green-guards core word-processing content (para/heading/bold-run/bookmark) through real DOCX write+read, and an `#[ignore]`'d reference-fixture test exercises the comprehensive fixture. **First real bug found *and fixed* via the harness:** the reference round-trip surfaced a content-loss gap at `blk0005/i0000/i0000` — DOCX export's `emit_char_props` silently omitted several run properties the importer reads (highlight, letter-spacing, all-caps, scale, shadow, kerning, complex/East-Asian fonts+size, run background, language), so a run formatted *only* by one of them exported with an empty `<w:rPr>`, collapsed to a plain run, and merged with neighbours (dropping the `StyledRun` and adjacent text). Fixed by making `emit_char_props` symmetric with the reader (new `docx/write/run_props.rs` + `run_props_tests.rs`); regression-locked by `docx_round_trip_preserves_secondary_run_formatting`. A **second** harness-found bug followed at `blk0026/i0001`: footnote-reference export hard-coded an explicit `<w:vertAlign superscript>` the source model never had — fixed by dropping the redundant run property and always emitting the `FootnoteReference`/`EndnoteReference` character styles (which carry the superscript, as Word does). **The full comprehensive reference fixture now round-trips with zero model divergence**, so `docx_reference_round_trip_is_stable` is promoted from `#[ignore]` gap-finder to a permanent green guard. **Canonicalization deepened** beyond paragraph/run depth: table interiors (`model/tables.rs` — rows/cells/spans/cell-blocks) and document metadata (`model/meta.rs` — core + custom + extended Dublin Core) are now walked, with model tests for each. **All three round-trip *shapes* are wired:** DOCX (`loki-ooxml`), **ODF/ODT** (`loki-odf/tests/conformance_round_trip.rs` — core + table + secondary formatting + bookmarks, all clean), and **sheet/XLSX** (new `sheet-model` feature + `NormalizedModel for Workbook` in `sheet/mod.rs`; `loki-ooxml/tests/conformance_xlsx_round_trip.rs` — values/formula/style/column-width, clean). The genuine DOCX export→re-import tests (metadata, math, comments) are **folded through the differ** (whole-model first-divergence backstop on top of their bespoke asserts). *Finding:* the `round_trip*.rs` / `round_trip_conformance*.rs` files (the "25") are **import-only smoke tests**, not export→re-import round-trips, so the differ does not apply to them. **Remaining (smaller):** ODS/sheet export gaps as they surface; promote `loki-acid`'s catalog into a shared fixture corpus (overlaps B-8/B-9). | **Largely done** |
| B-8 | Shared crate / M1 | `appthere-conformance` absent; `loki-acid` is Text-coupled | **Large** | ✅ **Built 2026-07-12** — crate live with all three axes; the corpus layer (`corpus` module) holds the promoted `TC-*` catalog, `Format`/`Severity`, and the `Fixture`/`Consumer` traits; golden discovery generalised (`golden::discovery`); the SSIM/ΔE differ was promoted earlier (B-4). `loki-acid` is the first consumer (re-export shims + trait impls + `AcidConsumer`). | |
| B-9 | Corpus / §9 | 141 TC cases flat; not organised feature×format×axis; ODP/ODG/PPTX importers absent | **Medium** | ✅ **Built 2026-07-12** — on-disk layout `fixtures/<format>/<id>.<ext>` + `goldens/<format>/<id>/`; `corpus::manifest::MANIFEST` records feature/axes/reference(+version)/tolerance-override-with-justification per fixture; catalog queryable by feature × format × severity. *(Count correction: the per-format tables sum to 139, not 141 — asserted in `counts_match_plan_totals`.)* ODP/ODG/PPTX importer gap = Presentation scope, unchanged. | |
| B-10 | Fonts / D4 | Gelasio (≈Georgia) not bundled; substitution suite absent | **Small** | ✅ **Resolved → bundle Gelasio**; still author the substitution suite | **Resolved** |
| B-11 | CI / §11 M6 | ✅ **Unblocked** — Spec 01 pipeline now built (9 gates in `rust.yml`) | **Sequencing** | Land schema+round-trip as hard gates (and visual post-calibration) into the now-populated lint job, same script-gate pattern | **Unblocked** |

---

## 8. Sequencing & dependency on Spec 01

Spec 02 §0 depends on Spec 01's CI pipeline and enforcement primitives. When this
inventory was filed those were "specified but not yet implemented"; **as of
2026-06-29 they are built** — [`spec-01-audit-report.md`](spec-01-audit-report.md)
§4 lists nine structural gates live in `rust.yml`. So **M6 (CI integration) is no
longer blocked**: the schema and round-trip gates plug into the populated lint job
exactly like the gates already there (a script under `scripts/`, one `run:` step).
The axes themselves (M1–M5) build against the existing `loki-acid` regardless.
Recommended order, lowest-risk-first and independent of B-1:

1. **B-8 + B-6** — crate skeleton + schema axis (no GPU, `xmllint` ready). = M1+M2.
2. **B-7** — normalized round-trip differ + deepened canonicalizer, all 3 shapes
   (DOCX/ODT/XLSX) wired, genuine round-trip tests folded in. = M3. ✅ *largely done.*
3. **B-1 (resolved → `vello_cpu`) + B-2/B-5** — CPU rasterizer + ODF golden generation. = M4.
4. **B-4 + B-3** — extended differ + calibration record. = M5.
5. **B-11** — wire the schema + round-trip gates into the lint job (Spec 01's
   pipeline is now built, so this is ready whenever M2/M3 land). = M6.

---

## 9. Acceptance check against the audit-first method (§3.1)

- ✅ Existing ACID corpus inventoried (141 `TC-*`; 22 P0 / 82 P1 / 47 P2; 7 asset
  fixtures, importer coverage per `loki-acid/README.md`).
- ✅ Each case-family mapped to axes (catalog↔corpus §9; round-trip tests §4;
  visual scaffold §5).
- ✅ Import/export crates inspected (`loki-ooxml`, `loki-odf`, `loki-opc`).
- ✅ Render path inspected — **CPU-rasterizer gap surfaced as B-1** (the spec's
  determinism premise vs. the actual GPU-only path).
- ✅ Environment capabilities verified (`xmllint`/`soffice` present; no GPU).
- ✅ Triage table populated **except** Priority (maintainer, per Spec 01 D1).
- ✅ **No code changed.**

**Honest scope note:** this is the read-only inventory only. Building
`appthere-conformance`, vendoring schemas, the CPU render backend, golden
generation, and calibration (M1–M6) are deferred pending maintainer triage of the
table above (B-1, B-10 resolved; B-11 unblocked; B-2…B-9 still open). The earlier
caveat that M6 was gated on un-built Spec 01 infrastructure **no longer applies**
— that pipeline now exists (9 gates).
