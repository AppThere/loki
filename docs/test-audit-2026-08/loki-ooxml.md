# Test-efficacy audit — loki-ooxml

Audited 2026-08-25. Scope: all `#[test]`s in `loki-ooxml` (in-crate `#[cfg(test)]` mods,
sibling `*_tests.rs` files, and `tests/` integration suites). Method: read every test
file, traced suspicious tests into the production code they call, and applied the
mutation thought-experiment ("would this test fail if the production function were
stubbed / its guard inverted?").

All paths are relative to `/home/kdesltd/project/loki/loki-ooxml/` unless absolute.

---

## Category A — cannot fail

### A-1. DOCX export smoke tests assert only ZIP magic bytes
- **Crate:** loki-ooxml
- **File:** `src/docx/export.rs:122-192`
- **Tests:** `export_empty_document_produces_zip`, `export_document_with_heading_and_para`, `export_bullet_list_produces_zip`, `export_ordered_list_produces_zip`
- **Category:** A (with a B flavour for the latter three: the names claim heading/list export)
- **Confidence:** HIGH
- **Defect:** The only assertion in each test is that the output begins with `PK`, so any export that produces *any* ZIP — including one that drops the heading, the list, or all content — passes.
- **Evidence:**
  ```rust
  let bytes = buf.into_inner();
  assert_eq!(&bytes[..2], b"PK");
  ```
  (`export_document_with_heading_and_para` and both list tests end with exactly this;
  no part is opened, no XML inspected.)
- **Mutation check:** stub `write_block`'s `Block::BulletList`/`OrderedList` arms
  (`src/docx/write/document.rs:203-209`) to emit nothing → all four tests still pass.
- **Fixed test would assert:** open `word/document.xml` (and `word/numbering.xml`) from the
  ZIP and assert the heading paragraph carries `w:pStyle`/`w:outlineLvl`, and the list
  items carry `w:numPr` with a `numbering.xml`-declared `numId` — or round-trip and
  assert on the model. (See also C-2: these are the *only* tests of the
  `Block::BulletList`/`OrderedList` export path.)

### A-2. Error-Display tests assert strings of values the test constructs
- **Crate:** loki-ooxml
- **File:** `src/docx/mapper/error.rs:69-84`; duplicated at `src/docx/mapper/mod_tests.rs:84-98` and again in the doc-tests on the enum itself
- **Tests:** `missing_required_element_display`, `invalid_value_display`, `missing_required_element_message`, `invalid_value_message`
- **Category:** A (borderline — technically fails if the `#[error]` format string is edited, but exercises only the `thiserror` derive, no crate logic; and the same assertion exists three times)
- **Confidence:** HIGH
- **Defect:** Tests construct a `MapperError` and assert its `Display` contains the field they just put in — no production code path that *produces* these errors is exercised (except `Pipeline`, covered by `missing_office_document_rel_yields_pipeline_error`, which is fine).
- **Evidence:**
  ```rust
  let e = MapperError::MissingRequiredElement { element: "w:body" };
  assert!(e.to_string().contains("w:body"));
  ```
- **Fixed test would assert:** feed a body-less `DocxDocument` through `map_document` and
  assert the returned error is `MissingRequiredElement { element: "w:body" }` — i.e. test
  the production site that raises the error, not the derive. (No production path currently
  raises `MissingRequiredElement`/`InvalidValue` under test at all — see C-5.)

---

## Category B — doesn't test what it claims

### B-1. Import-export-import "divergence" round-trips are blind to total first-export loss
- **Crate:** loki-ooxml
- **File:** `tests/conformance_round_trip.rs:79` (`docx_round_trip_preserves_core_content`), `:113` (`docx_round_trip_preserves_secondary_run_formatting`), `:154` (`docx_round_trip_preserves_emboss_imprint_and_char_border`)
- **Category:** B (symmetric round-trip methodology)
- **Confidence:** HIGH
- **Defect:** `round_trip_divergence(seed)` compares `a = import(export(seed))` with `b = import(export(a))` — both sides are *post-export*, so a property silently and idempotently dropped on the **first** export makes `a == b` and the test **passes**. The named property ("bold", "highlight", "emboss/imprint/bdr") "surviving intact" is exactly what is *not* checked.
- **Evidence:**
  ```rust
  fn round_trip_divergence(seed: &Document) -> Option<Divergence> {
      let a = import(export(seed));
      let b = import(export(&a));
      first_divergence(&canonicalize_document(&a), &canonicalize_document(&b))
  }
  ```
  The sibling file `tests/conformance_p0_round_trip.rs:7-12` states the weakness
  explicitly: "compares two *consecutive* import-export cycles (`a` vs `b`) and is
  therefore blind to a property that is dropped on the **first** export".
- **Mutation check:** stub `emit_char_props` to skip `w:highlight` entirely →
  `docx_round_trip_preserves_secondary_run_formatting` (whose doc-comment says it guards
  precisely that regression) still passes: the highlight is lost in `a` and equally lost
  in `b`. The regression is only re-caught indirectly, by
  `docx_reference_round_trip_is_stable` (fixture-anchored `a`) *if* the reference fixture
  carries the property — it carries highlight/letter-spacing but **not**
  emboss/imprint/`w:bdr`, so the emboss test's claim is fully unguarded at this level
  (the writer-side `run_props_tests.rs` and reader-side `props_rpr.rs` unit tests are the
  only real anchors for those).
- **Fixed test would assert:** after **one** `import(export(seed))`, assert the property's
  value against the seed (the pattern `conformance_p0_round_trip.rs` and
  `docx_round_trip_preserves_floating_text_box` already use — the latter's comment even
  says "assert directly (not only via divergence) so a silent downgrade … fails loudly").
  Keep the divergence check as a backstop, not the sole assertion.

### B-2. Same divergence-only pattern on the XLSX side
- **Crate:** loki-ooxml
- **File:** `tests/conformance_xlsx_round_trip.rs:42` (`xlsx_round_trip_preserves_core_content`)
- **Category:** B
- **Confidence:** HIGH (mechanism), lower severity
- **Defect:** Same `a` vs `b` post-export comparison; the bold style, percent format, formula, and column width it claims to "preserve" can all be dropped on first export without failing this test.
- **Evidence:** same `round_trip_divergence` shape as B-1 (`tests/conformance_xlsx_round_trip.rs:33-37`).
- **Mitigation:** `tests/xlsx_round_trip.rs::test_xlsx_round_trip_basic` and
  `test_xlsx_column_widths_round_trip` assert the same properties directly against the
  seed, and `tests/conformance_xlsx_p0_round_trip.rs` covers formulas — so the *specific*
  seeds here are covered elsewhere; the test is redundant-but-misleadingly-named rather
  than the only guard.
- **Fixed test would assert:** seed-anchored values after one cycle (as `xlsx_round_trip.rs` does), or delete in favour of those.

### B-3. `layout_assigns_header_footer_per_page` never checks *which* header variant a page got
- **Crate:** loki-ooxml
- **File:** `tests/round_trip.rs:297-349`
- **Test:** `layout_assigns_header_footer_per_page`
- **Category:** B
- **Confidence:** HIGH
- **Defect:** The doc-comment and inline comment claim "the first page gets the first-page header and subsequent pages get the default header (gap #5)", but every assertion is only non-emptiness (`!p1.header_items.is_empty()`, `header_height > 0`) — the test passes identically if page 1 receives the *default* header or if all pages receive the same variant.
- **Evidence:**
  ```rust
  // Page 1 gets the first-page header variant (titlePg=true, header_first is set).
  let p1 = &paginated.pages[0];
  assert!(!p1.header_items.is_empty(), "page 1 should have header items (first-page header variant)");
  ```
  The fixture makes discrimination trivially available — the first-page header text is
  `"First Page Header"` vs default `"Test Document Header"` (`tests/helpers/mod.rs:169-197`)
  — and it is never used.
- **Mutation check:** make the layout engine ignore `header_first` and always use
  `header` → test passes.
- **Fixed test would assert:** the glyph/text content of page 1's `header_items` contains
  "First Page Header" and page 2's contains "Test Document Header".

### B-4. `export_p0` / `export_p1` round-trips assert presence, not fidelity
- **Crate:** loki-ooxml
- **Files:** `tests/export_p0.rs:17` (`export_round_trip_p0_features`), `tests/export_p1.rs:14` (`export_round_trip_p1_features`)
- **Category:** B (loose assertions)
- **Confidence:** MEDIUM
- **Defect:** After the export→re-import, the tests assert only that *some* `Inline::Link`, *some* `Inline::Image`, *some* footnote / first-line indent / tab stop / cell background exists. A hyperlink whose URL was rewritten to `"#"`, an image with zero-byte data, a first-line indent of the wrong size, or a cell shaded the wrong colour all pass.
- **Evidence:**
  ```rust
  let has_link = all_blocks.iter().any(|b| block_has_link(b));
  assert!(has_link, "Hyperlink should survive round-trip");
  ```
- **Mitigation:** `docx_reference_round_trip_is_stable` (`tests/conformance_round_trip.rs:303`)
  is fixture-anchored and runs the whole-model differ over the same reference document, so
  value-level loss *is* caught there for canonicalised properties. These two tests add
  little beyond it in their current form.
- **Fixed test would assert:** the re-imported link's `target.url == "https://example.com"`, the image target/data URI matches, `indent_first_line ≈ 36.0`, the two cell shading colours are `FF0000`/`00FF00`.

### B-5. `unknown_list_id_falls_back_to_a_bullet_list` never asserts the bullet
- **Crate:** loki-ooxml
- **File:** `tests/list_style_round_trip.rs:186-203`
- **Category:** B
- **Confidence:** MEDIUM
- **Defect:** Name and doc-comment promise "must still export as a list (the default-bullet fallback)"; the body asserts only that both orphan paragraphs are still list items sharing one id — a fallback to a *numbered* list, or to a broken empty definition, passes.
- **Evidence:**
  ```rust
  assert_eq!(members[0].0, members[1].0, "the orphaned list's items must share one list");
  ```
  — the only assertion after the `expect("orphans must stay list items")`.
- **Fixed test would assert:** the round-tripped catalog entry for the orphans' list id has
  `ListLevelKind::Bullet { .. }` at level 0.

### B-6. `xml_util_tests::external_entities_are_never_resolved` asserts against `Debug` output
- **Crate:** loki-ooxml
- **File:** `src/xml_util_tests.rs:168-184`
- **Category:** B (weak instrument, works today)
- **Confidence:** MEDIUM
- **Defect:** The XXE posture is asserted by formatting the whole parsed document with `{:?}` and substring-matching `"root:"` / `"&xxe;"`. `!text.contains("root:")` only fails if the runner's `/etc/passwd` was actually fetched *and* the Debug repr prints it — on a machine where the file read fails silently the first assertion is vacuous. The second assertion (`&xxe;` stays literal) is the load-bearing one and is sound.
- **Evidence:**
  ```rust
  let text = format!("{doc:?}");
  assert!(!text.contains("root:"), ...);
  assert!(text.contains("&xxe;"), ...);
  ```
- **Fixed test would assert:** extract the run text via the model (as other tests do) and
  assert it equals exactly `"&xxe;"` — one positive equality instead of one vacuous
  negative + one substring.

---

## Category C — not exhaustive (material gaps)

### C-1. XLSX import has zero error-path tests
- **Crate:** loki-ooxml
- **Files:** `src/xlsx/import.rs`, `src/xlsx/import_worksheet.rs`, `src/xlsx/import_styles.rs`, `src/xlsx/export.rs`, `src/xlsx/export_xml.rs` — 0 `#[test]` in all five; the integration suites (`tests/xlsx_round_trip.rs`, `tests/conformance_xlsx_*.rs`, `src/xlsx/vba_tests.rs`) are all happy-path.
- **Category:** C
- **Confidence:** HIGH
- **Defect:** No test feeds the XLSX importer a corrupt ZIP, a package missing `xl/workbook.xml`, a workbook whose `r:id` points at a missing worksheet part, or malformed sheet XML. The DOCX side has exactly these tests (`missing_office_document_rel_yields_pipeline_error`, nesting-depth guards, malformed-table import); the XLSX side has none, so its error handling (`Result` paths in `XlsxImport::run`) is entirely unverified — any of those paths could panic or silently return an empty workbook.
- **Fixed test would assert:** each malformed input returns a typed `OoxmlError` (not a panic, not `Ok(empty)`), mirroring `tests/round_trip.rs::test_table_with_invalid_attributes` and `mod_tests.rs::missing_office_document_rel_yields_pipeline_error`.

### C-2. `Block::BulletList` / `Block::OrderedList` export path has no efficacy coverage
- **Crate:** loki-ooxml
- **File:** production `src/docx/write/document.rs:203-209` (+ `src/docx/write/numbering.rs`); only tests are the ZIP-magic ones in A-1
- **Category:** C
- **Confidence:** HIGH
- **Defect:** The importer never produces `Block::BulletList`/`OrderedList` (it emits `StyledPara` + `list_id`), so `tests/list_style_round_trip.rs` never touches these arms; the only tests naming them assert ZIP magic. Stubbing both arms to emit plain paragraphs fails no test. These blocks arrive from other front-ends (pandoc-style models), so this is a live export surface.
- **Fixed test would assert:** exporting a `BulletList`/`OrderedList` document yields paragraphs with `w:numPr` referencing a `numId` declared in `word/numbering.xml` with the right `numFmt` — or a round-trip asserting the re-imported items have `list_id`/`list_level` set and distinct ids for the two list kinds.

### C-3. `w:evenAndOddHeaders` / `w:titlePg` explicit-off values untested in the settings reader
- **Crate:** loki-ooxml
- **File:** `src/docx/reader/settings.rs:73-100` (tests)
- **Category:** C
- **Confidence:** HIGH
- **Defect:** `mirrorMargins` gets both on and `w:val="0"` cases, but `evenAndOddHeaders` is tested only present→true and `titlePg` only absent→false. The `is_none_or(|v| !matches!(...))` off-branch for those two elements is uncovered — inverting it (treating `w:val="0"` as on) fails no test.
- **Fixed test would assert:** `<w:evenAndOddHeaders w:val="0"/>` → `false`, `<w:titlePg/>` → `true` and `<w:titlePg w:val="false"/>` → `false`. Similarly `toggle_prop` (`src/docx/reader/util.rs:60-83`) never tests `"on"`/`"off"`/`"true"`.

### C-4. PPTX import has no malformed-package tests
- **Crate:** loki-ooxml
- **File:** `src/pptx/import_tests.rs` (all four tests build one well-formed synthetic package)
- **Category:** C
- **Confidence:** MEDIUM
- **Defect:** No test covers a `.pptx` missing `ppt/presentation.xml`, a `p:sldId` whose `r:id` resolves to nothing, or malformed slide XML — the importer's error paths are unexercised. (The `unsupported_shapes_are_reported` warning test is good; it is the only non-happy case.)
- **Fixed test would assert:** each returns a typed error or a documented skip-with-warning, not a panic.

### C-5. `MapperError::MissingRequiredElement` / `InvalidValue` are never produced by any tested path
- **Crate:** loki-ooxml
- **File:** `src/docx/mapper/error.rs` (see A-2), production `src/docx/mapper/mod.rs`
- **Category:** C
- **Confidence:** MEDIUM
- **Defect:** The only tests touching these variants construct them by hand. If no production code can actually reach them, they are unreachable-and-unmarked (rule 6 territory); if some path can, that path is untested. Either way the current tests establish nothing about the mapper's failure behaviour beyond the `Pipeline` wrapper.
- **What would settle it:** grep the mapper for constructor sites; either add a test driving each real site (e.g. a document with no `w:body`) or mark the variants as reserved.

---

## Per-crate stats

- **Test files read:** 77 (40 in `src/` with `#[test]`s or `#[cfg(test)]` mods, 36 in `tests/`, plus `tests/helpers/mod.rs`)
- **Tests examined:** ~381 `#[test]` functions
- **Findings:** A: 2 (covering 8 test fns) · B: 6 (covering 9 test fns) · C: 5 gaps
- **Confidence split:** HIGH 9, MEDIUM 4

## Healthy areas

The crate's test discipline is, on the whole, unusually strong; the findings above are
the exceptions. Notably good:

- **`tests/table_style_round_trip.rs`** — exemplary inversion discipline: four distinct
  padding values so side-permutation bugs discriminate, both-signs glyph-shift assertions
  with an explicit "phenomenon is reachable" pre-check, and fixture-precondition asserts.
- **`tests/page_style_names_round_trip.rs`** + `src/docx/page_style_part_tests.rs` +
  `src/docx/reader/page_style_part_tests.rs` — geometry-invariance proven against a
  stripped control with a non-vacuousness check, a stale part rejected end-to-end through
  a re-zipped package, and a writer/reader compatibility *property* test born from a real
  cross-format probe.
- **`tests/wt_literal_newlines.rs`, `tests/doc_defaults_inheritance.rs`** — each fix
  carries its own inversions ("an explicit `<w:br/>` must still break", "basedOn must
  still win"), exactly the guard-mutation the evidence rules ask for.
- **`src/docx/repair/repair_tests.rs`** + `tests/repair.rs` — every repair has an
  analyze-only (no-edit) counterpart, byte-for-byte clean-input preservation, and
  conservative-skip cases (foreign children, comments).
- **`src/docx/omml/omml_tests.rs`** — round-trips are anchored by hand-written OMML
  fixtures with exact-equality MathML assertions before the stability check, avoiding the
  B-1 trap.
- **`tests/schema_validation.rs`** — validates real exports against the vendored ECMA-376
  XSDs *and* proves the gate can fail (`deliberately_malformed_document_fails_the_gate`).
- **The reader/mapper unit-test pairs** (`document_run_tests`, `props_tests`,
  `numbering_tests`, `styles_tests`, `table_tests`, `images_tests`) assert exact model
  values from exact XML, including PUA bullet normalisation, vMerge geometry, and
  warning emission for dangling references.
- **Security posture tests** — nesting-depth rejection (tables and `w:sdt`), EMU clamping,
  and the XXE literal-entity check (see B-6 for its one weak half).
