# Test-efficacy audit — export/conversion/container crates

Scope: loki-epub, loki-pdf, loki-opc, loki-convert, loki-print, loki-headless,
loki-graphics. Static analysis only; every HIGH finding was verified by reading
the production code the test calls. Categories: **A** = cannot fail,
**B** = doesn't test what it claims, **C** = material coverage gap.

All paths are relative to `/home/kdesltd/project/loki/`.

---

## loki-graphics

### G-1 — A, HIGH — discarded `matches!` is a no-op assertion
- `loki-graphics/src/shape.rs:225`, test `geometry_shape_constructor`
- The kind-check is a bare expression statement, so it can never fail.
- Evidence:
  ```rust
  assert_eq!(s.transform.rotation_deg, 0.0);
  matches!(s.kind, ShapeKind::Geometry(_));   // result discarded
  ```
  `matches!` returns a `bool` that is silently dropped (bool is not
  `#[must_use]`, so no lint fires). If `Shape::geometry` produced a
  `ShapeKind::Group`, the test still passes.
- Fix: `assert!(matches!(s.kind, ShapeKind::Geometry(_)));`

### G-2 — C, MEDIUM — constructor-echo-only suite
- `loki-graphics/src/{drawing,geometry,path,shape,style,text}.rs` (14 tests)
- Every test constructs a value and reads back the fields just set. Acceptable
  for a pure data-model crate, but `RectF::contains` edge semantics
  (inclusive right/bottom edge) and `Path` builder are the only real logic and
  are lightly covered; `preset.rs` and `id.rs` have no tests (both trivial).
  No action urgent; noted for completeness.

---

## loki-convert

### CV-1 — A/B, HIGH — "round trip" that checks only ZIP magic bytes
- `loki-convert/tests/matrix_round_trips.rs:100`, test `ods_and_xlsx_round_trip`
- The test's only assertions on both conversion outputs are the two-byte ZIP
  signature; no workbook content is verified in either direction, so it is a
  round trip in name only.
- Evidence:
  ```rust
  let xlsx = convert(Format::Ods, &ods, Format::Xlsx, ...).unwrap();
  assert_eq!(&xlsx.bytes[..2], b"PK");
  let back = convert(Format::Xlsx, &xlsx.bytes, Format::Ods, ...).unwrap();
  assert_eq!(&back.bytes[..2], b"PK");
  ```
  Mutation: an `export_sheet` stub that writes an empty workbook ZIP — or an
  `import_sheet` stub returning `Workbook::new()` — passes. The fixture is also
  an empty `Worksheet::new("Sheet1")`, so even the sheet has nothing to lose.
- Fix: give the fixture a named sheet with at least one string and one numeric
  cell, re-import the final ODS with `OdsImport`, and assert sheet name and
  cell values survive (mirroring `docx_to_odt_to_docx_preserves_text`).

### CV-2 — B, MEDIUM-HIGH — macro *preservation* branch never exercised (one-sided guard)
- `loki-convert/src/pipeline.rs:77,91` guard `macros_preserved` /
  `preserve`; only test: `matrix_round_trips.rs:162`
  (`dropping_macros_on_conversion_warns`)
- The suite proves macros are *dropped with a warning* on DOCX→ODT, but no test
  performs the identity ODT→ODT (or ODS→ODS) conversion with macros to assert
  they survive and no warning is emitted — the guard is never false in any test.
- Evidence: hardcoding `let macros_preserved = false;` in `pipeline.rs`
  (deleting the preservation path) fails no test.
- Fix: build an ODT with a `MacroPayload`, convert ODT→ODT, assert the payload
  part is present in the output ZIP and `out.warnings` is empty.

### CV-3 — B, MEDIUM — `title` option set but never asserted
- `loki-convert/tests/matrix_round_trips.rs:69`,
  test `docx_to_pdf_emits_requested_profile`
- The test passes `title: Some("Print run".into())` but only asserts the PDF
  header and the `PDF/X-4` marker; the title-override line in
  `pipeline.rs:71-73` (`doc.meta.title = Some(title.clone())`) can be deleted
  with no test failing anywhere in the workspace.
- Fix: assert the PDF's Info/XMP contains `Print run` (it is written via
  `dc:title` / `/Title`), or drop the option from the fixture so the test
  doesn't imply coverage it lacks.

### CV-4 — B, MEDIUM — EPUB conversion asserts container, not content
- `loki-convert/tests/matrix_round_trips.rs:82`,
  test `docx_to_epub_produces_epub_container`
- Asserts `PK` magic and the literal `application/epub+zip` somewhere in the
  bytes; never checks that "Quarterly report" (the fixture text) reached the
  EPUB. A pipeline that exported an empty `Document` to EPUB passes.
- Fix: open the ZIP, read `EPUB/content.xhtml`, assert the fixture heading and
  paragraph text are present.

### CV-5 — B, MEDIUM — Fountain→PDF asserts only the header
- `loki-convert/tests/text_import_sources.rs:48`, test `fountain_converts_to_pdf`
- `assert!(out.bytes.starts_with(b"%PDF"))` is the sole assertion; a Fountain
  importer that returned an empty document (one blank page) would pass, unlike
  the sibling Markdown test which re-imports and counts list items.
- Fix: assert page count ≥ 1 *and* that a text operator / embedded font exists
  (e.g. `FontFile2`), or reuse the `declared_page_count` helper pattern from
  loki-pdf's `pdf_structure.rs`.

### CV-6 — C, LOW — `Format::from_extension` has no unit test
- `loki-convert/src/format.rs:51`
- Only two extensions (`docx`, `pptx`) are exercised indirectly via the
  loki-headless CLI tests; a typo'd mapping for e.g. `md`/`odg` would ship.
- Fix: a table-driven test over all extensions plus a `None` case.

Healthy: `tests/page_geometry_conformance.rs` is exemplary — asymmetric
fixtures chosen so swaps are visible, explicit polarity tests
(`mirrored_margins_cross_unchanged` tests the *unmirrored* document too),
an empty-catalog case targeting the exact historical bug, and a fixed-point
double-crossing test. `matrix.rs::matrix_matches_the_spec` asserts both
directions of the matrix *and* pins `supported_pairs().len()` (floor and
ceiling).

---

## loki-opc

### O-1 — B, HIGH — core-properties round-trip assertion is feature-gated off
- `loki-opc/tests/package_tests.rs:9` (`test_package_round_trip`),
  gate at line 32
- The test's headline value — that `core_properties` (title) survives
  write→open — is inside `#[cfg(feature = "serde")]`, and `serde` is **not** a
  default feature (`Cargo.toml [features] default = ["std"]`). In a default
  `cargo test` run the test asserts only that `/word/document.xml` exists.
  No other test in the crate asserts any core-properties *value* round-trips
  (`core_properties_part_is_typed_correctly` checks only the content type).
- Evidence:
  ```rust
  #[cfg(feature = "serde")]
  {
      let title = pkg_read.core_properties().unwrap().title...
      assert_eq!(title, "Test Title");
  }
  ```
  The gated block does not use serde at all — `core_properties()` is
  unconditionally available — so the gate looks accidental. The test also
  contains leftover debug `println!("DEBUG RAW ZIP ZIP: ...")` / `PROPS` output.
- Fix: delete the `#[cfg]` gate (the assertion compiles without serde), remove
  the debug prints, and extend to a second field (e.g. `creator`).

### O-2 — C, HIGH — reader error paths have zero tests
- `loki-opc/src/zip/read.rs:41` (`MissingContentTypes`), `:77`
  (`UnsupportedCompression`), `:99` (`UnknownMediaType`)
- Grep across `loki-opc/tests/` and all `#[cfg(test)]` modules finds no test
  producing any of these three errors; every reader test feeds a conformant
  archive. Deleting the `UnsupportedCompression` check (letting e.g. bzip2
  entries fall through to a zip-crate error or silent skip) or the
  `UnknownMediaType` check fails no test.
- Fix: three small tests building hostile ZIPs in-memory: one with no
  `[Content_Types].xml`, one with a bzip2-compressed entry, one with a part
  whose extension has no Default/Override.

### O-3 — C, MEDIUM — `PartName::extension()` bug not catchable by current tests
- `loki-opc/src/part/name.rs:97-99`; tests `part_name_extension_and_rels_name`
  (`tests/api_tests.rs:43`) and `test_extension`
- `extension()` does `self.0.rsplit_once('.')` over the **whole path**, so a
  dotted directory segment misbehaves: `/v1.0/data` → `Some("0/data")` instead
  of `None`. All existing fixtures use dot-free directories, so the tests
  cannot catch it. This matters because `extension()` feeds
  `ContentTypeMap::resolve` for every part.
- Fix (test side): add `/v1.0/data` → `None` and `/a.b/c.xml` → `Some("xml")`
  cases (they fail today, exposing the production bug — fix is to rsplit the
  final segment only).

### O-4 — C, LOW-MEDIUM — untested compat/strict surface
- `loki-opc/src/compat/part_names.rs` `normalize_percent_encoding` (called at
  `zip/read.rs:88`) has no test; the `strict` feature (deviations become
  errors) is never compiled in any test configuration, so its behavioral
  contract is unverified.

Healthy: `zip/limits.rs` budget tests exercise both rejection variants with
exact error matching; `zip/read.rs` transcode tests lock the audit-S-3
odd-length fix; `api_tests.rs` part-name grammar tests invert every rule, and
the UTF-16 package test builds a real BOM'd archive and asserts exact
transcoded bytes. `core_properties_part_is_typed_correctly` is a model
regression test (asserts the raw `[Content_Types].xml` override *and* the
reopened resolution).

---

## loki-pdf

### P-1 — B, HIGH — transparency *flattening* asserted only by mask absence
- `loki-pdf/src/image.rs:277`, test `x4_keeps_a_soft_mask_but_x1a_x3_flatten_it`;
  production compositing at `image.rs:173-181` (`chan` closure)
- The test claims X-1a/X-3 "flatten" transparency, but asserts only
  `alpha_flate.is_none()` — the actual compositing-over-white math is never
  checked against a pixel value.
- Evidence: stub the flatten branch of
  ```rust
  let chan = |c: u8| { ... if flatten { n * af + (1.0 - af) } else { n } };
  ```
  to plain `n` (i.e. drop the compositing, keep dropping the mask) and every
  test in the workspace passes — yet X-1a output would render semi-transparent
  pixels at full ink.
- Fix: decode `flat.entries()[0]`'s `cmyk_flate` (inflate) for the alpha=128
  red pixel and assert the CMYK sample equals red-over-white (≈ 50% magenta/
  yellow), and differs from the unflattened bank's sample.

### P-2 — C, HIGH — level→flatten wiring has no test (uninvertible guard)
- `loki-pdf/src/build.rs:37`:
  `images.set_flatten_transparency(!options.level.allows_transparency());`
- `allows_transparency()` is unit-tested, and `ImageBank` flattening is
  unit-tested with a hand-set flag, but no test exports a translucent image
  through `export_document` at X-1a vs X-4 — so removing the `!` (or the line)
  is caught by nothing. This is the exact seam between two well-tested halves.
- Fix: integration test exporting a doc with a translucent data-URI image at
  X-1a (assert no `/SMask` in the output) and X-4 (assert `/SMask` present).

### P-3 — B, MEDIUM — clip test asserts operator presence, not clipping
- `loki-pdf/src/page_tests.rs:48`, test `clipped_group_emits_clip_operators`
- `stream.contains("re")` is satisfied by the child rect's own `re` even with
  clipping deleted; `contains('W')` / `contains('q')` are single-character
  scans over the whole stream; nothing checks the clip rect's coordinates, the
  y-flip (`page_h - (oy + y + h)`), or that the clip precedes the children and
  `Q` follows them.
- Fix: assert the exact clip prelude substring
  (e.g. `"0 80 20 20 re\nW n"` for the fixture) and its index is below the
  child's fill index, with `Q` after both.

### P-4 — B/C, MEDIUM — rotation test never checks the matrix values
- `loki-pdf/src/page_tests.rs:94`, test `rotated_group_emits_transform_and_children`
- Asserts `cm`/`q`/`Q`/`f` presence only. `rotated_group_ctm` itself is
  excellently tested (`page_rotate_tests.rs`), but the *call* in `render_item`
  — passing the right origin/degrees/page-height — is unverified: swapping
  `content_width`/`content_height` or dropping `oy` at the call site passes.
- Fix: parse the six numbers before `cm` in the stream and compare to
  `rotated_group_ctm(20, 30, 90, 10, 4, 200)`.

### P-5 — B, LOW — setter test over-claims
- `loki-pdf/src/options.rs:168`, test `with_icc_profile_sets_the_dest_output_profile`
- Asserts a one-line builder stored its argument; "sets the DestOutputProfile"
  is actually established by `lib.rs::supplied_icc_profile_is_embedded_as_dest_output_profile`.
  Harmless, but the name claims the integration behaviour. Rename or fold.

Healthy: `tests/pdf_structure.rs` re-parses structure (declared `/Count` vs
counted leaf pages, `startxref` offset dereferenced against raw bytes, all
three PDF/X levels iterated); `fonts_subset_tests.rs` proves variable-font
instancing *moves outlines* via a control-point fingerprint (regular vs
wght=700) rather than just "bytes differ"; `page_rotate_tests.rs` verifies the
CTM contract at all four corners against the on-screen transform;
`page_tests.rs` notdef filtering asserts both the empty-bank and
mixed-run cases; color tests pin the black/white/red anchors of RGB→CMYK.

---

## loki-epub

### E-1 — C, MEDIUM-HIGH — `heading_level` style mapping has no test
- `loki-epub/src/inlines.rs:230-242`; sole caller `content.rs:121`
- The `Heading1`…`Heading6` / `"Heading 1"` / `Title` / clamp-to-1..=6 mapping
  that turns styled paragraphs into `<h1>`–`<h6>` (and thus TOC entries) is
  untested anywhere: all heading tests use the numeric `Block::Heading`
  variant. Breaking the prefix parse (e.g. `starts_with("heading")` typo) or
  the clamp fails nothing.
- Fix: unit cases for `"Heading3"`, `"Heading 2"`, `"heading9"` (→6), `"Title"`
  (→1), `"Body"` (→None), plus one `StyledPara`-with-Heading1-style content
  test asserting `<h1>` and a TOC entry.

### E-2 — C, HIGH — `render_image` fallback branches untested
- `loki-epub/src/images.rs:62-71`
- The external-URL branch (reference without packaging) and empty-URL branch
  (fall back to escaped alt text) are never rendered in any test — the images
  tests only check `decode_data_uri` returns `None`, not what the renderer
  does with that `None`. A regression that packaged external URLs (or emitted
  `<img src=""/>`) passes.
- Fix: content-render tests: an `https://` image asserting the `<img>` src
  survives verbatim and `rendered.images` stays empty; an empty-target image
  asserting alt text is emitted and no `<img>` appears.

### E-3 — C, MEDIUM — `has_extended_metadata` is dead and untested
- `loki-epub/src/opf_meta.rs:136-142`
- Doc says "Exposed for callers/tests that want to assert metadata richness";
  grep finds zero callers and zero tests workspace-wide. Its OR-chain would
  silently rot (e.g. a new DublinCore field). Either test it or delete it
  (per the project's own dead-code rules, deletion is cleaner).

### E-4 — C, LOW-MEDIUM — inline formatting variants unasserted
- `loki-epub/src/inlines.rs:61-124` (`render_inline`),
  `:171-211` (`render_styled_run`)
- No test asserts output for `Emph`/`Strong`/`Underline`/`Code`/`Quoted`/
  `Link`, nor `render_styled_run`'s bold/italic/underline tag emission and its
  reverse-order closing (a nesting bug would produce malformed XHTML). The
  integration well-formedness checks would catch *mismatched* tags only if such
  content were in a fixture — it is in none.
- Fix: one table-driven content test covering each inline wrapper and one
  StyledRun with bold+italic+underline asserting exact nested markup.

Healthy: `tests/epub_structure.rs` re-opens the OCF, verifies mimetype-first +
Stored compression, drives quick-xml over every part, and checks packaged image
bytes and manifest entries; `tests/path_a_lists.rs` asserts nested `<ul>`
placement *inside* the parent `<li>`, closure before the following paragraph,
balanced tag counts, and the unknown-list-id fallback; `content_tests.rs`
covers field resolution three ways including the renders-nothing negative;
`tables_tests.rs` includes the colgroup-omitted negative, override-beats-column
(with the stale-value negative), and colspan column-index tracking.

---

## loki-print

### PR-1 — C, HIGH — the entire IPP wire path is untested
- `loki-print/src/client.rs:82-183` — `print_pdf`, `job_state`,
  `wait_for_completion`, `check_status`, `job_id_of`, `job_state_of`,
  `job_attribute`
- All 7 existing tests cover pure helpers (state enum mapping, URI parse,
  options→attributes). Nothing exercises a request/response cycle: no mock IPP
  server, no assertion on the encoded Print-Job operation (that the
  document-format attribute, payload, and options actually land in the
  request), and no response parsing test (`job_id_of` group filtering,
  non-success `check_status` → `PrinterStatus`, `wait_for_completion`'s
  Canceled/Aborted → `JobFailed` vs `JobTimeout` distinction — the poll loop
  also sleeps 2s per iteration, making it untestable as written).
- Note: this is the inverse of the "mock bypasses wire encoding" smell — there
  is no mock at all, so the wire layer has zero coverage rather than fake
  coverage. `IppRequestResponse` can be constructed/parsed in-process via the
  `ipp` crate, or a one-shot TCP listener can capture the encoded request.
- Fix: (a) serialise the built Print-Job operation to bytes and assert the
  operation-id, document-format, and job attributes are present; (b) feed
  hand-built Get-Job-Attributes responses to `job_state_of`/`job_id_of`;
  (c) inject the poll interval/clock so `wait_for_completion`'s three exits
  are each reachable in a fast test.

### PR-2 — C, LOW-MEDIUM — single-range `page-ranges` encoding branch untested
- `loki-print/src/options.rs:155-158`
- The bare-`RangeOfInteger`-vs-`Array` choice is only tested with two ranges
  (`"1-3,5"` → Array); the single-range bare-value branch — RFC-visible on the
  wire — is never hit. Fix: add `page_ranges: Some("2-4")` asserting the value
  is not an `Array`.

Healthy: `parse_page_ranges` tests invert every rejection rule (zero page,
inverted, junk, empty, empty segment) and confirm the error surfaces at
attribute-build time; the state-mapping test covers all seven RFC 8011 states.

---

## loki-headless

### H-1 — C, MEDIUM — `render` and `print` subcommands have no tests
- `loki-headless/src/commands.rs` / `main.rs`; tests only in `tests/cli.rs`
- The CLI's four subcommands are convert/render/print/formats; only convert
  and formats are tested. `render` is fully offline (Parley → pdf-writer per
  the spec) and testable today; even `print`'s argument validation
  (bad `--printer` URI → typed error, exit code) is uncovered.
- Fix: a `render` smoke test mirroring `convert_docx_to_pdf_via_cli`, and a
  `print --printer "not a uri"` test asserting non-zero exit + the
  InvalidPrinterUri message.

Healthy: the three existing tests run the real binary; the gate test asserts
the *message*, and `formats_lists_the_matrix` includes the negative
(`!stdout.contains("pptx")`).

---

## Per-crate statistics

| Crate | Test files read | Tests examined | A | B | C |
|---|---|---|---|---|---|
| loki-epub | 10 | 42 | 0 | 0 | 4 (E-1..E-4) |
| loki-pdf | 9 | 34 | 0 | 3 (P-1, P-3, P-5) | 2 (P-2, P-4*) |
| loki-opc | 5 | 28 | 0 | 1 (O-1) | 3 (O-2..O-4) |
| loki-convert | 4 | 19 | 1* (CV-1) | 4 (CV-2..CV-5) | 1 (CV-6) |
| loki-print | 2 | 7 | 0 | 0 | 2 (PR-1, PR-2) |
| loki-headless | 1 | 3 | 0 | 0 | 1 (H-1) |
| loki-graphics | 6 | 14 | 1 (G-1) | 0 | 1 (G-2) |
| **Total** | **37** | **147** | **2** | **8** | **14** |

\* CV-1 is counted as A (its assertions cannot fail under content-destroying
mutations) though it also reads as B; P-4 straddles B/C — counted once as C.

## Healthy areas (summary)

The suite's strong spine is real: loki-convert's page-geometry conformance
suite and loki-pdf's structural re-parse tests are the best in scope — both
were clearly written by inverting guards and choosing fixtures whose corruption
is visible. loki-epub's integration tests re-open the container and re-parse
every XML part rather than substring-matching the writer's own buffer.
loki-opc's part-name grammar and zip-bomb budget tests invert every rule.
The systemic weak spots are concentrated at **seams**: spreadsheet conversion
content (CV-1), the PDF level→flatten wiring (P-2), the IPP wire layer (PR-1),
and a feature-gate that silently disabled loki-opc's only core-properties
value assertion (O-1).
