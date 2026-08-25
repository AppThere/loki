# Test-efficacy audit — loki-odf

Audited: all 41 test files (19 `tests/` integration binaries incl. shared `helpers.rs`,
22 `src/` unit-test files — sibling `*_tests.rs` and inline `#[cfg(test)]` mods).
Method: read every test; for each suspicious test, read the production code it exercises
(`ods/import.rs`, `ods/export.rs`, `ods/export_content.rs`, `ods/import_helpers.rs`,
`package.rs`, `odt/write/*`, `odt/mapper/*`, `odt/reader/*`). Static analysis only; no
files modified, no builds run.

---

## Findings

### F1 — ODS "round trip" silently masks a total metadata loss
- **Crate/file:** loki-odf, `/home/kdesltd/project/loki/loki-odf/tests/ods_round_trip.rs:14-17` (setup), assertions from line 94
- **Test:** `test_ods_round_trip`
- **Category:** B (doesn't test what it claims) + C (real gap behind it)
- **Confidence:** HIGH (production code read)
- **Defect:** The test sets `workbook.meta = DocumentMeta { title, creator }` in the fixture but never asserts metadata after re-import — and the production ODS pipeline drops metadata entirely, so the only test named "round trip" passes while a whole model field is lost.
- **Evidence:**
  - Test sets `title: Some("Test Spreadsheet")`, `creator: Some("Loki Test Builder")` (lines 15-17); the assertion block (lines 94-147) checks sheets/cells/styles/formula only — no `imported_workbook.meta` assertion.
  - `src/ods/import.rs:275` — importer hardcodes `meta: DocumentMeta::default()`.
  - `grep meta src/ods/export.rs src/ods/export_content.rs` → no hits: ODS export writes no `meta.xml` at all.
  - CLAUDE.md's tech-debt table documents metadata round-trip for the **Loro/ODT** path only; the ODS-side loss is not marked (`// TODO`) anywhere in `ods/`.
- **Fixed test would assert:** `imported_workbook.meta.title == Some("Test Spreadsheet")` and `.creator == Some("Loki Test Builder")` — which today fails, forcing either implementation of ODS `meta.xml` write/read or a mechanically-marked deferral (drop the field from the fixture + `TODO(ods-meta)`).

### F2 — Same test canonises a formula-representation asymmetry
- **Crate/file:** `/home/kdesltd/project/loki/loki-odf/tests/ods_round_trip.rs:62,134`
- **Test:** `test_ods_round_trip`
- **Category:** B (wrong methodology — asserts the mutation, not the invariant)
- **Confidence:** HIGH (both converters read)
- **Defect:** The seed cell formula is `"SUM(A2:B2)"` but the test asserts the re-imported value is `"=SUM(A2:B2)"` — the round trip changes the model's spelling of the same fact and the test enshrines it, so the model has two representations for one formula (leading-`=` and bare) that only converge after a save+reopen.
- **Evidence:**
  - Line 62: `cell_c2.formula = Some("SUM(A2:B2)".to_string());`
  - Line 134: `assert_eq!(c2_imp.formula.as_deref(), Some("=SUM(A2:B2)"));`
  - `src/ods/import_helpers.rs:64-96` `clean_ods_formula` → `format!("={result}")` always prepends `=`; `src/ods/export.rs:113-118` `to_ods_formula` strips a leading `=`. Stable at fixpoint, but the first trip is not identity and no test defines which spelling is canonical.
- **Fixed test would assert:** a single canonical model spelling (assert seed == re-import), or explicitly test the normalisation in both converters' unit tests and store the canonical form in the fixture.

### F3 — Committed-fixture leak test passes vacuously when the fixture is absent
- **Crate/file:** `/home/kdesltd/project/loki/loki-odf/tests/synthetic_style_leak.rs:189-199`
- **Test:** `committed_fixture_round_trips_without_synthetic_ids`
- **Category:** A (can pass with zero assertions executed)
- **Confidence:** HIGH
- **Defect:** If `appthere-conformance/fixtures/odt/para-carlito.odt` is not present, the test prints "skipping" and `return`s Ok — a green result indistinguishable from a real pass, with no mechanical marking (`#[ignore]`, panic, or generation step).
- **Evidence:**
  ```rust
  let Ok(bytes) = std::fs::read(path) else {
      eprintln!("skipping: {path} not present");
      return;
  };
  ```
- **Fixed test would assert:** fail (or `#[ignore]` with a comment naming the generator) when the fixture is missing, so a tree that never generated fixtures cannot report this coverage as passing. (Per the project's own rule 6: the marking must be mechanical.)

### F4 — Symmetric-only round trips for several writer surfaces; schema gate covers a minimal document
- **Crate/files:**
  - `/home/kdesltd/project/loki/loki-odf/tests/revision_round_trip.rs` (both tests)
  - `/home/kdesltd/project/loki/loki-odf/tests/math_round_trip.rs` (both tests)
  - `/home/kdesltd/project/loki/loki-odf/tests/odt_export_round_trip.rs`: `comments_round_trip` (792), `headers_and_footers_round_trip` (557), `multi_column_section_round_trips` (863), `extended_dublin_core_round_trips` (762), `emboss_and_char_border_round_trip_through_odt` (336), `floating_text_box_round_trips_through_odt` (400)
  - `/home/kdesltd/project/loki/loki-odf/tests/schema_validation.rs` (`sample_document`, lines 36-80)
- **Category:** B (methodology — a bug symmetric in writer+reader passes) / C (schema-coverage gap)
- **Confidence:** HIGH for the coverage facts; the risk itself is the known symmetric-round-trip blind spot
- **Defect:** Tracked changes, comments (`office:annotation`), embedded math objects, header/footer variants, `style:columns`, character emboss/borders, extended Dublin Core, and floating text boxes are verified **only** by export→re-import through this crate's own writer+reader pair; the RELAX-NG schema gate validates only `sample_document` (one heading, one bold run, one 2×2 table — no meta fields, lists, changes, comments, frames, or columns). A writer and reader that agreed on a wrong element/attribute spelling for any of these would pass the entire suite while every third-party application saw garbage — the exact failure mode the suite itself documents at `odt_export_round_trip.rs:1405-1413` ("a reader and writer that agreed on some other spelling would round-trip perfectly … and be wrong for every other application") and fixes *only* for `style:page-usage`.
- **Evidence:** `revision_round_trip.rs` / `math_round_trip.rs` contain no XML/bytes assertion and no schema validation; `schema_validation.rs::sample_document` exercises none of the listed constructs.
- **Fixed tests would assert:** either extend `schema_validation.rs`'s document to carry each of these constructs (cheapest — the validator is already wired and has a deliberate-failure control), or add one byte-level spelling assertion per surface in the style of `mirrored_page_usage_survives_an_odt_round_trip`.

### F5 — `styles.xml`-absent fallback branch has zero coverage
- **Crate/file:** production `/home/kdesltd/project/loki/loki-odf/src/package.rs:126-128`; test files `src/package_tests.rs`, `tests/malformed_odt_test.rs`, `tests/helpers.rs`
- **Category:** C (untested branch)
- **Confidence:** HIGH (all zip-builders inspected)
- **Defect:** `OdfPackage::open` deliberately treats `styles.xml` as optional (`.unwrap_or_else(|| b"<office:document-styles/>".to_vec())`), but every test package that opens successfully includes `styles.xml` — the fallback is only "reached" in tests that fail earlier for other reasons (mimetype ordering), so deleting the fallback (making styles mandatory) would break real ODF 1.1 files while no test fails.
- **Evidence:** `helpers::build_odt_zip` and `build_odt_zip_no_content` both write `styles.xml`; `package_tests::minimal_zip` writes it; `security_limits::build_odf_zip` writes it; `import_tests::build_odt_zip` writes it.
- **Fixed test would assert:** a package with `mimetype`+`manifest`+`content.xml` and **no** `styles.xml` opens Ok and imports with default styles.

### F6 — ODS content-reader error paths and value-type matrix untested
- **Crate/file:** `/home/kdesltd/project/loki/loki-odf/tests/ods_round_trip.rs` (only functional ODS test), `tests/security_limits.rs` (clamps only), `src/ods/import.rs`, `src/ods/export_content.rs:163-177`
- **Category:** C
- **Confidence:** MEDIUM-HIGH
- **Defect:** ODS has exactly one happy-path integration test plus clamp tests. Untested: malformed spreadsheet XML inside a valid package (truncated `content.xml`, mismatched tags — the ODT importer has 5 such tests, ODS has none); the boolean cell in F1's fixture asserts only the string value `"true"` and nothing distinguishes the `office:value-type="boolean"` writer arm from the string arm; covered/spanned ODS cells; no ODS schema validation (schema gate is ODT-only).
- **Evidence:** `export_content.rs` has three value-type arms (boolean/float/string); the test's C1 assertion (`assert_eq!(c1_imp.value, "true")`) passes even if booleans were exported as strings, since `import.rs:193` falls back between `office:value` and `office:boolean-value` transparently.
- **Fixed tests would assert:** `Err(_)` for truncated/mismatched ODS content.xml; and, for the boolean cell, an assertion on the exported XML (`office:value-type="boolean"`) or a discriminating model field.

### F7 — Stale doc comment describes a different malformation than the body performs
- **Crate/file:** `/home/kdesltd/project/loki/loki-odf/tests/malformed_odt_test.rs:66-73`
- **Test:** `content_xml_invalid_xml_returns_error`
- **Category:** B (minor — doc/body mismatch)
- **Confidence:** HIGH
- **Defect:** The doc comment says the test omits a namespace declaration to force an XML-level error over a "missing required child of office:body"; the body actually tests a mismatched closing tag (`</office:document-WRONG>`). The behaviour tested is fine; the description tests something else, and the "missing required child" case it describes exists nowhere in the suite.
- **Evidence:** comment vs. `let bad_xml = b"...<office:document-content></office:document-WRONG>";`
- **Fixed test:** correct the comment, and (optionally) add the actually-described case.

### F8 — `parse_odf_border` has no malformed-input coverage
- **Crate/file:** `/home/kdesltd/project/loki/loki-odf/src/odt/mapper/props/tests.rs:416-433`
- **Tests:** `parse_odf_border_solid_black`, `parse_odf_border_none_returns_none`
- **Category:** C
- **Confidence:** MEDIUM (parser not fully traced)
- **Defect:** Only one well-formed value and the literal `"none"` are tested. Untested: partial shorthands (`"1pt"`, `"solid"`), unknown style keywords, missing colour, garbage — the inputs a foreign writer actually produces. Same for the `fo:border` shorthand path used by cell/paragraph props.
- **Fixed test would assert:** documented behaviour (None vs. defaulted Border) for each malformed shape, so the fallback policy is pinned rather than incidental.

### F9 — Presence-only assertions in the ODF-1/ODF-3 gap tests
- **Crate/file:** `/home/kdesltd/project/loki/loki-odf/tests/round_trip.rs:193-213` (`odf1_paragraph_border_mapped`), `:248-269` (`odf3_paragraph_background_color_mapped`)
- **Category:** B (weak — asserts on presence, not value)
- **Confidence:** HIGH
- **Defect:** The fixture declares `fo:border="1pt solid #000000"` and `fo:background-color="#FFFFCC"`, but the tests assert only `border_top.is_some()` / `background_color.is_some()` — a mapper that parsed every border as 0pt dotted white, or every background as black, passes. (Contrast the cell-props sibling `odf_cell_props_mapped`, which checks numeric padding values.)
- **Fixed test would assert:** width ≈ 1pt, style Solid, colour #000000; background hex #FFFFCC — the discriminating values the fixture already carries.

---

## Per-crate stats

| Metric | Count |
|---|---|
| Test files read | 41 (19 integration incl. helpers.rs, 22 unit) |
| Tests examined | ~297 (106 integration, 191 unit) |
| Findings — A (cannot fail) | 1 (F3 — vacuous-pass mechanism) |
| Findings — B (doesn't test what it claims) | 5 (F1, F2, F4, F7, F9) |
| Findings — C (material gaps) | 4 (F5, F6, F8 + the gap half of F1/F4) |
| HIGH-confidence findings | 7 (F1, F2, F3, F4, F5, F7, F9) |
| MEDIUM-confidence findings | 2 (F6, F8) |

## Healthy areas

This is one of the strongest test suites I have audited; the findings above are the
residue, not the norm. Notably good practice, worth imitating elsewhere:

- **Anti-vacuity guards:** `synthetic_style_leak.rs` pairs its negative byte assertion
  with `the_fixture_actually_sets_a_synthetic_parent`, proving the exercised path exists;
  `schema_validation.rs` includes `deliberately_malformed_content_fails_the_gate`, a
  control proving the validator can fail.
- **Polarity/inversion coverage:** `an_ordinary_document_carries_no_page_usage_at_all`,
  `an_unhighlighted_run_gains_no_background`, `no_columns_means_no_column_layout`,
  `content_without_a_list_style_yields_none`, `parentless_style_keeps_none_when_no_default_style_exists` — guards are tested false, not just true.
- **Discriminating values:** the table-border baking tests use six distinct edge widths so
  position (not mere presence) is pinned; auto-style tests use 1/2/3/4pt tuples explicitly
  to catch order slips; `mirrored_page_usage` asserts bytes *and* model.
- **Stability-vs-fidelity awareness:** `conformance_round_trip.rs` documents that
  import-export-import comparison cannot see first-export losses and adds the fidelity
  half (`a_named_highlight_survives_odt_export`) separately.
- **Error paths (ODT side):** malformed-package, nesting-depth, allocation-bomb, and
  version-strictness paths are all covered with typed-error matching, including clamp
  values and post-clamp cursor positions.

The concentration of weakness is the **ODS half** of the crate (F1, F2, F6): it has a
fraction of the ODT suite's rigor, and its single round-trip test both misses a real
data loss (metadata) and canonises a representation change (formula `=` prefix).
