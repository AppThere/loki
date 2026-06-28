<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 01 — Audit Report (M1)

| | |
|---|---|
| **Status** | Draft — inventory complete, awaiting maintainer triage |
| **Companion to** | [`spec-01-codebase-audit-and-architecture.md`](spec-01-codebase-audit-and-architecture.md) |
| **Architecture ADR** | [`0009-target-architecture.md`](0009-target-architecture.md) (M2) |
| **Method** | Read-only audit pass. **No production code changed.** |
| **Snapshot** | branch `claude/adr-docs-setup-ogwz5a`, 2026-06-28 |
| **Corpus** | 664 tracked `.rs` files · 25 workspace crates · Rust 2024 edition |

This is the **M1** deliverable mandated by §3 / §8 of Spec 01: a complete,
triageable inventory across every §4.1 smell category, the dedicated 1280px-class
trace (§4.2), and the raw dependency graph. Per **D1 (surface-and-triage)** the
**Priority** column of the triage table (§3) is left blank for the maintainer.
Per the working method, **no fixes are applied in this pass** — enforcement (M3)
and fixes (M4) follow maintainer triage.

> **On counts.** Numbers below come from deterministic `git grep` / `cargo
> metadata` / line-count scans, with `#[cfg(test)]` modules, `*_tests.rs`,
> `tests/`, `benches/`, `examples/`, and the vendored `patches/` tree excluded
> from "production library" totals unless stated. Test code is explicitly exempt
> (§4.1) — its `unwrap()`s and large files are acceptable. Where a heuristic can
> over- or under-count, the report says so rather than rounding up.

---

## 1. Executive summary

The codebase is in **substantially better shape than the spec's framing
implies**. Most of Loki's stated conventions already largely hold; the problem
is that *almost none of them are mechanically enforced* (§13), so they regress
silently. The headline findings:

- **The 1280px class is real and load-bearing (A-1).** `window_width` is a
  `Signal<f32>` initialised to `1280.0` in `editor_state.rs` and **never written
  again**. Meanwhile the *actually measured* viewport width lives in a *separate*
  value (`scroll_metrics.client_width`, sourced from `get_client_rect()`) used
  for reflow. Hit-testing and pointer math read the stale 1280 default; reflow
  reads the real width. **On any window that is not 1280 px wide, the two
  diverge** and click-to-caret mapping drifts. This is exactly the
  "temporary-decision-became-permanent" bug the spec names.
- **Error handling is already disciplined.** Genuine library-runtime
  `unwrap()`/`expect()`/`panic!` is in the *single digits* (A-5), all
  safe-by-construction; ad-hoc `Box<dyn Error>`/`String` errors are effectively
  absent (A-10). `thiserror` is the norm.
- **One real layering violation (A-8):** `loki-renderer` (render layer) imports
  `appthere_ui` (ui layer) — a single uphill edge, for design tokens. The UDOM
  waist otherwise holds: no format crate (`loki-odf`/`loki-ooxml`) leaks into
  layout or render.
- **Two clean, mechanical hygiene gaps:** the entire `loki-opc` crate (26 files)
  plus a stray root `scratch.rs` are **missing SPDX headers** (A-3/A-4); 38
  production files exceed the 300-line ceiling (A-2, already tracked in
  `docs/audit-2026-06.md`).
- **Enforcement is the actual deliverable.** CI today runs only `cargo fmt
  --check` + default `clippy -D warnings` + build/test. There is **no**
  `clippy.toml`, no `disallowed-methods`, no file-ceiling gate, no SPDX gate, no
  dependency-direction gate, and no dylint (A-13). Every convention in §6 of the
  spec is currently unenforced.

Net: this is a **gates-first** job, not a sweep. The smells are few and mostly
mechanical; the value is in making them un-reintroducible (M3).

---

## 2. The 1280px class — dedicated trace (§4.2)

### 2.1 Every hardcoded viewport/page/window dimension

`git grep` for the literal `1280` across non-test, non-patch `.rs` yields exactly
**two** hits, and both point at the same root:

| Location | Code | Role |
|---|---|---|
| `loki-text/src/routes/editor/editor_state.rs:177` | `window_width: use_signal(\|\| 1280.0_f32)` | **The source.** Default never overwritten. |
| `loki-text/src/editing/hit_test.rs:17` | `//! window_inner_width_px defaults to 1280 px` | Doc comment acknowledging the assumption. |

The *consequences* fan out through the centring formula
`(window_width − page_width_px) / 2.0` (pages are flex-centred), duplicated at:

| Location | Expression |
|---|---|
| `editor_pointer.rs:41` | `((window_width − client_width_px) / 2.0).max(0.0)` |
| `editor_pointer.rs:98` | `(window_width() − page_width_px).max(0.0) / 2.0` |
| `editor_pointer.rs:167` | `(window_width() − page_width_px).max(0.0) / 2.0` |
| `editor_pointer.rs:260` | `(window_width() − page_width_px).max(0.0) / 2.0` |
| `editor_spell_panel.rs:51` | `(window_width − MENU_WIDTH_PX − EDGE_MARGIN_PX).max(0.0)` |
| `hit_test.rs:16` (doc) | `canvas_origin.x = (window_inner_width_px − page_width_px) / 2.0` |

Adjacent magic-dimension literals found in the same scan (lower-severity, but in
the same class — a numeric literal in a layout/render path that is neither a
named `const` nor viewport-sourced):

| Location | Literal | Note |
|---|---|---|
| `editor_canvas.rs:383` | `if h > 1.0 { h } else { 800.0 }` | Fallback page height; magic. |
| `loki-spreadsheet/.../editor_inner.rs:74` | `const MAX_COL_PX: f64 = 800.0` | Named, but app-local; fine. |
| `docx/mapper/document.rs:488-489` | `header: 720, footer: 720` | Twips (½ inch) — intrinsic OOXML default, **legitimate**; should carry a named const + comment. |
| `presentation-model/presentation.rs:39` | `STANDARD_4_3 = Size::new(720.0, 540.0)` | Named const — **legitimate** intrinsic. |

### 2.2 Origin & what *should* supply the value

The mechanism (confirmed by reading the code, not inferred):

1. `window_width` is initialised to `1280.0` and there is **no writer** anywhere
   in `loki-text/src` — `git grep 'window_width\.set\|window_width ='` returns
   only the `use_signal` initialiser and read sites.
2. The editor *does* measure the real viewport: `editor_inner.rs:612` awaits
   `evt.get_client_rect()` and stores `rect.size.width` into
   `scroll_metrics.client_width` (`editor_canvas.rs:250`,
   `editor_inner.rs:615`). That measured width feeds `reflow_width_px`
   (`editor_canvas.rs:394`) — i.e. **rendering and reflow already use the real
   width.**
3. Hit-test / pointer / spell-panel math instead reads the *stale* `window_width`
   signal. So the rendered page is centred using the measured width while the
   click-to-caret transform assumes 1280 — they agree only when the window
   happens to be 1280 px wide.

**What should supply the value:** the live measured viewport width
(`scroll_metrics.client_width`), the page geometry (`tokens::PAGE_WIDTH_PX` /
`state.page_width_px`), the zoom state, and DPI — bundled into **one**
`Viewport` / `LayoutContext` value threaded through render *and* hit-test, with a
single `canvas_origin_x()` helper replacing the six duplicated centring
expressions. This is **D4** of the spec and the precondition for Spec 03
(Responsive). It is also the sanctioned alternative the `no_hardcoded_layout_dims`
dylint (§6.2) must point authors toward.

---

## 3. Smell inventory by category (§4.1)

### 3.1 Hardcoded dimensions / magic numbers — see §2 (A-1).

### 3.2 Leaked `unwrap()` / `expect()` / `panic!` (A-5)

Raw production counts (after stripping `#[cfg(test)]` modules): **20 `unwrap()`,
21 `expect()`, 3 `panic!`**. But categorising each by *where it actually lives*
deflates the genuine risk sharply:

| Bucket | Count | Verdict |
|---|---|---|
| Doc-comment examples (`//! … .unwrap()`) | ~6 | Acceptable; gate must exempt doc-tests. |
| `build.rs` (`env::var(...).unwrap()`) | 6 | Build scripts — conventional, not library code. |
| `benches/` `panic!` | 3 | Bench harness — test-equivalent, exempt. |
| `Box<dyn Iterator>` / doc `Box<dyn Error>` | 3 | False positives (not error handling). |
| **Genuine library-runtime `unwrap()`** | **~7** | Safe-by-construction; see below. |

The genuine library `unwrap()`s, all guarded by a local invariant rather than
reachable from untrusted input:

- `loki-opc/src/part/name.rs:110`, `relationships/location.rs:17,22` —
  `PartName::new_unchecked(format!(...)).unwrap()` on strings known-valid by
  construction.
- `loki-opc/src/zip/read.rs:120,122` — `strip_suffix(".rels").unwrap()` guarded
  by a prior `.ends_with(".rels")`.
- `loki-opc/src/package.rs:174` — `core_properties.as_mut().unwrap()` after an
  ensured insert.
- `loki-ooxml/src/xlsx/export.rs:371` — `rows.remove(&r).unwrap()` on a key just
  proven present.
- `loki-doc-model/src/loro_bridge/comments.rs:94` — `write_document_comments(…).unwrap()`.

The `expect()` set is similar (e.g. `loki-spreadsheet` ×7, `loki-templates` ×4,
mostly `OnceLock`/regex-compile-once patterns). **None are reachable from a
public API with attacker-controlled input.** Recommendation: convert each to `?`
+ typed error or a documented `// invariant:`/`expect("…")` justification, then
let the `clippy.toml` `disallowed-methods` gate (§6.1) hold the line — with
target-level exemptions for `build.rs`, `benches/`, doctests, and `#[cfg(test)]`.

### 3.3 `unsafe` blocks (A-7)

Exactly the three documented Android `NativeActivity` crates contain `unsafe`:
`loki-text`, `loki-presentation`, `loki-spreadsheet` (`src/lib.rs`). Each call
site carries a `// SAFETY:` justification (e.g. `loki-text/src/lib.rs:4`,
`loki-presentation/src/lib.rs:45`, `loki-spreadsheet/src/lib.rs:51`). **No
undocumented `unsafe` exists** in production crates. The gap is structural, not
local — see A-7: those three crates simply *omit* `#![forbid(unsafe_code)]`
rather than `#![deny]` + a scoped `#[allow(unsafe_code)]` on the FFI item, and
there is no checked-in allow-list enumerating them.

### 3.4 File-ceiling violations (A-2)

**38 production `.rs` files** exceed 300 lines (excluding `tests/`, `*_tests.rs`,
and `patches/`; one further 300+ file — `odt/mapper/props/tests.rs` — is a test
file and exempt). Worst offenders:

| Lines | File |
|---|---|
| 1982 | `loki-layout/src/para.rs` |
| 1953 | `loki-layout/src/flow.rs` |
| 1554 | `loki-odf/src/odt/reader/styles.rs` |
| 1494 | `loki-odf/src/odt/reader/document.rs` |
| 1244 | `loki-spreadsheet/src/routes/editor/editor_inner.rs` |
| 1209 | `loki-ooxml/src/docx/reader/document.rs` |
| 1073 | `loki-ooxml/src/docx/write/document.rs` |
| 1046 | `loki-text/src/routes/editor/editor_inner.rs` |
| 984 | `loki-layout/src/resolve.rs` |
| 948 | `loki-vello/src/scene.rs` |

This overlaps the existing split-pass backlog in `docs/audit-2026-06.md`
(finding Q-1) and the CLAUDE.md tech-debt table. **No new finding beyond
confirming the count and feeding it to the §6.3 ceiling gate** (which needs a
reviewed exceptions file seeded from this list so CI distinguishes known debt
from new violations).

### 3.5 SPDX header issues (A-3, A-4)

**27 production files lack the Apache-2.0 SPDX header entirely:**

- **All 26 files of `loki-opc/src/`** (`lib.rs`, `package.rs`, `error.rs`,
  `compat/*`, `content_types/*`, `core_properties/*`, `part/*`,
  `relationships/*`, `zip/*`, `constants.rs`). The crate predates the convention
  and was never back-filled.
- **`scratch.rs`** at the repository root — a stray scratch file (see A-4).

**Zero files** have the header present-but-not-on-line-1. So the fix is pure
insertion (line 1), and the `spdx_header_line_one` dylint (§6.2) will hold it.

### 3.6 Inconsistent error handling (A-10)

**Effectively none.** The only `Box<dyn …>` matches are a `Box<dyn Iterator>`
(`loki-layout/src/result.rs:55`, not an error type) and a `Box<dyn
std::error::Error>` inside a doc example (`docx/mapper/mod.rs:259`). No `String`-
typed library errors surfaced. Loki already uses `thiserror` enums per crate.
**This category is essentially clean** — the gate's job is to keep it that way,
not to remediate.

### 3.7 Dead / unreachable code (A-4)

- **`scratch.rs`** (root, 206 bytes, also SPDX-less) — a stray file, not a
  workspace member, almost certainly dead. Candidate for deletion.
- Root-level stray artifacts (not `.rs`, but worth a maintainer glance):
  `diff_flow.txt`, `scratch.rs`, `iris*.png/log`, `output.png`,
  `test_output*.{png,txt}`, `test_log*.txt` — committed debug/output dumps that
  bloat the tree. Flagged for triage, not auto-removed.
- A deeper dead-`pub` / orphaned-module sweep needs `cargo +nightly udeps` /
  `warnings(dead_code)` over an `--all-features` build and is **deferred** to a
  fix-pass with the gate live (honest scope note: not exhaustively done here).

### 3.8 Duplication (A-14)

- **The centring formula** (§2.1) — six near-identical copies of `(window_width −
  …)/2.0`. Folds into the `Viewport::canvas_origin_x()` helper. (Counted under
  A-1.)
- **Android `android_main` entry point** — `loki-presentation/src/lib.rs` and
  `loki-spreadsheet/src/lib.rs` are near-identical (logger tag + a
  `recent_documents` wiring line differ); `loki-text` is a third near-copy. The
  shared body (`AndroidLogger` setup, `set_android_app`, `loki_i18n::init`,
  `init_android` FFI) belongs in a `loki-app-shell` helper, leaving each crate a
  thin per-app shim. Real, low-risk consolidation.

### 3.9 `HACK` / `TODO` / `FIXME` / `XXX` debt (A-11)

Production (non-test, non-patch): **49 `// TODO`**, **59 `// COMPAT`**, **0
`HACK`/`FIXME`/`XXX`**. The `COMPAT(dioxus-native)` annotations are *sanctioned*
by CLAUDE.md (they mark Blitz/Dioxus-Native limitations) and are not debt to
remove — but they *should* be inventoried so each can be re-validated as the
pinned Dioxus `=0.7.9` moves. The 49 `TODO`s should each become a tracked
decision or a `TODO(<topic>)` with an owner. The absence of `HACK`/`FIXME`/`XXX`
is a good sign of discipline.

### 3.10 Naming inconsistency (A-12)

Mostly consistent: import/export config is uniformly `*Options`
(`OdtImportOptions`, `XlsxImportOptions`, `LayoutOptions`, `PdfXOptions`,
`EpubOptions`, …). The one real divergence:

- **`*Props` is overloaded.** It means both Dioxus *component* props
  (`AtTitleBarProps`, `DocumentViewProps`, …) **and** document-model *formatting
  property bags* (`ParaProps`, `CharProps`, `RunProps`, `CellProps`,
  `TableProps`, `ShapeProps`). Two unrelated concepts share a suffix across the
  model↔ui boundary. Low priority, but a candidate to rename the model bags to
  `*Format`/`*Attrs` to disambiguate.
- Minor: `DocxSettings` / `DocumentSettings` use `*Settings` where `*Options`
  is the norm — but `DocxSettings` maps to the OOXML `settings.xml` part, so the
  name is domain-justified. Leave as-is.

### 3.11 Layering violations (A-8, A-9)

Computed from `cargo metadata` against the target layers (full analysis in
[`0009-target-architecture.md`](0009-target-architecture.md)):

- **A-8 — `loki-renderer` → `appthere_ui` (uphill render→ui).** Single edge, via
  `use appthere_ui::tokens;` (`loki-renderer/src/document_view.rs:9`). The render
  layer pulls UI design tokens. Migration: relocate the consumed tokens to a
  foundation crate (or inject them through a render context). **The only true
  uphill edge in the workspace.**
- **A-9 — `loki-pdf` → `loki-layout` (classification, not a violation).** `loki-pdf`
  is an *export* crate but legitimately reuses layout for positioning, so it sits
  *above* layout, not in the io/serde waist. The ADR refines the layering to put
  layout-consuming exporters (`loki-pdf`) above layout and pure-serialisation
  exporters (`loki-epub`, which imports only model+primitives) in the waist.

No `model → render/layout/ui` edges exist; foundation crates remain leaves;
`loki-odf`/`loki-ooxml` do not leak into layout/render. The architecture is
**one edge away from clean**.

---

## 4. Enforcement gap (A-13)

| Convention claimed (CLAUDE.md / spec §6) | Enforced today? |
|---|---|
| `cargo fmt` | ✅ CI (`rust.yml` `fmt --check`) |
| `clippy -D warnings` (default lints) | ✅ CI |
| `clippy::pedantic` + curated allow-list | ❌ no `[workspace.lints]` |
| No `unwrap`/`expect`/`panic!` in lib code | ❌ no `clippy.toml` `disallowed-methods` |
| 300-line file ceiling | ❌ no script gate |
| SPDX header on line 1 | ❌ no gate / dylint |
| `forbid(unsafe_code)` + enumerated exceptions | ❌ no gate / allow-list |
| Acyclic downhill-only deps | ❌ no `cargo metadata` gate |
| `no_hardcoded_layout_dims` (1280 class) | ❌ no dylint |

CI = `.github/workflows/rust.yml` (one workflow: lint job + build-and-test job).
This table is the M3 work-list.

---

## 5. Triage table (§4.3) — Priority left blank for maintainer (D1)

| ID | Category | Location(s) | Count | Blast radius | Proposed fix | Priority |
|----|----------|-------------|-------|--------------|--------------|----------|
| A-1 | Hardcoded viewport dim (1280 class) | `editor_state.rs:177`; `editor_pointer.rs` ×4; `hit_test.rs`; `editor_spell_panel.rs` | 1 source + 6 derived | **Medium** — `loki-text` input/hit-test; behaviour-affecting | Introduce `Viewport`/`LayoutContext` (measured width + page geom + zoom + DPI); feed hit-test from `scroll_metrics.client_width`; single `canvas_origin_x()` helper (D4) | |
| A-2 | File-ceiling >300 | 38 production files (`para.rs` 1982 … `scene.rs` 948) | 38 | **Large but mechanical** | Continue split-pass (`docs/audit-2026-06.md` Q-1); seed ceiling-gate exceptions file | |
| A-3 | SPDX missing | all 26 `loki-opc/src/*` | 26 | **Small** | Insert header on line 1; `spdx_header_line_one` dylint | |
| A-4 | Dead/stray file | `scratch.rs` (+ root debug dumps) | 1 (+~8) | **Small** | Delete `scratch.rs`; maintainer triages root artifacts | |
| A-5 | Library `unwrap`/`expect` | `loki-opc` ×~6, `loki-ooxml/xlsx/export.rs:371`, `loki-doc-model/.../comments.rs:94`, `loki-spreadsheet`/`loki-templates` `expect` | ~7 genuine | **Small** | `?` + typed error or justify; `disallowed-methods` gate w/ target exemptions | |
| A-6 | `panic!` in production | none in lib (3 in `benches/`) | 0 | n/a | No action; gate exempts benches | |
| A-7 | `forbid(unsafe_code)` absent | `loki-text`, `loki-presentation`, `loki-spreadsheet` `lib.rs` | 3 (expected) | **Small** | `#![deny(unsafe_code)]` + scoped `#[allow]` on FFI item; checked-in allow-list | |
| A-8 | Layering: render→ui (uphill) | `loki-renderer/src/document_view.rs:9` | 1 edge | **Small–Medium** | Move consumed `appthere_ui::tokens` to a foundation crate or inject via render ctx | |
| A-9 | Layering classification | `loki-pdf` → `loki-layout` | 1 | **None (doc)** | Refine layers in ADR; exporter-above-layout tier | |
| A-10 | Inconsistent error handling | — | ~0 | **None** | Keep `thiserror` norm; gate locks it | |
| A-11 | TODO/COMPAT debt | 49 TODO / 59 COMPAT (prod) | 108 | **Medium (process)** | Inventory; convert TODO→tracked; re-validate COMPAT vs Dioxus pin | |
| A-12 | Naming: `*Props` overloaded | model bags vs Dioxus props | ~6 model bags | **Medium (rename)** | Optionally rename model `*Props`→`*Format`/`*Attrs` | |
| A-13 | Enforcement gap | `clippy.toml`, ceiling/SPDX/dep-direction gates, dylint all absent | — | **Foundational** | Implement §6 gates (M3) before bulk fixes (D2) | |
| A-14 | Duplication | centring math (A-1); `android_main` ×3 | 2 clusters | **Small–Medium** | `Viewport` helper; shared `loki-app-shell::android_main` | |

---

## 6. Raw dependency graph (input to M2)

Internal `normal`-kind edges from `cargo metadata --no-deps` (25 crates):

```
appthere-canvas        -> loki-render-cache
appthere-ui            -> loki-i18n
loki-app-shell         -> loki-i18n, loki-spell
loki-doc-model         -> loki-primitives
loki-epub              -> loki-doc-model, loki-primitives
loki-graphics          -> loki-primitives
loki-layout            -> loki-doc-model, loki-fonts, loki-primitives, loki-spell
loki-odf               -> loki-doc-model, loki-primitives, loki-sheet-model
loki-ooxml             -> loki-doc-model, loki-graphics, loki-opc,
                          loki-presentation-model, loki-primitives, loki-sheet-model
loki-pdf               -> loki-doc-model, loki-layout, loki-primitives
loki-presentation      -> appthere-ui, loki-app-shell, loki-doc-model, loki-fonts,
                          loki-graphics, loki-i18n, loki-layout, loki-odf, loki-ooxml,
                          loki-presentation-model, loki-renderer, loki-vello
loki-presentation-model-> loki-graphics
loki-renderer          -> appthere-canvas, appthere-ui, loki-doc-model,
                          loki-layout, loki-vello          # appthere-ui = A-8 uphill
loki-sheet-model       -> loki-primitives
loki-spreadsheet       -> appthere-ui, loki-app-shell, loki-doc-model, loki-fonts,
                          loki-i18n, loki-layout, loki-odf, loki-ooxml, loki-renderer,
                          loki-sheet-model, loki-vello
loki-templates         -> loki-doc-model, loki-ooxml, loki-primitives
loki-text              -> appthere-ui, loki-app-shell, loki-doc-model, loki-epub,
                          loki-fonts, loki-i18n, loki-layout, loki-odf, loki-ooxml,
                          loki-pdf, loki-renderer, loki-templates, loki-vello
loki-vello             -> appthere-canvas, loki-layout
```

Leaves (no internal deps): `loki-primitives`, `loki-fonts`, `loki-i18n`,
`loki-spell`, `loki-opc`, `loki-render-cache`. The graph is **acyclic**; the only
uphill edge against the target layering is `loki-renderer → appthere-ui` (A-8).

---

## 7. Acceptance check against M1 (§8)

- ✅ Every crate inspected (25/25; quantitative scans cover all 664 tracked `.rs`).
- ✅ All §4.1 categories surfaced and counted.
- ✅ Dedicated 1280px-class trace (§2) with origin + single-source-of-truth proposal.
- ✅ Raw dependency graph produced (§6).
- ✅ Triage table populated **except** the Priority column (D1).
- ✅ **No code changed** — read-only pass.

**Next:** maintainer fills Priority → M3 (enforcement gates land first, D2) →
M4 (fixes top-down, `Viewport` replaces the 1280 class). The deferred items
honestly flagged above (exhaustive dead-`pub` sweep §3.7; root-artifact triage
§3.7) are called out rather than silently skipped.
