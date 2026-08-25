# Test-efficacy audit — loki-app-shell, loki-spell, loki-spreadsheet, loki-primitives, loki-sheet-model, loki-presentation-model, loki-presentation, loki-fonts, loki-i18n, loki-templates

Audit date: 2026-08-24. Static analysis only (no builds/tests run). Every finding
marked HIGH was verified by reading the production code the test calls.

Categories: **A** = cannot fail, **B** = doesn't test what it claims,
**C** = material gap (untested error path / boundary / missing inversion).

---

## loki-app-shell

### AS-1 — `implausible_dimensions_are_rejected` reimplements the validation instead of calling it
- **File:** `loki-app-shell/src/window_geometry.rs:95-101`
- **Test:** `implausible_dimensions_are_rejected`
- **Category:** A — **HIGH**
- **Defect:** The test re-declares the bounds check locally and never calls `WindowGeometry::load`, so deleting or inverting the validation in `load` (lines 43-44) fails no test.
- **Evidence:
  ```rust
  // test body (window_geometry.rs:99-100):
  let sane = |v: f64| (MIN_DIMENSION_PX..=MAX_DIMENSION_PX).contains(&v);
  assert!(!(sane(parsed.width) && sane(parsed.height)));
  ```
  The identical closure exists in production `load()` (line 43). The test asserts its own copy against a value it constructed — a tautology w.r.t. production behaviour ("one fact, one derivation" violation).
- **Fixed test would assert:** `WindowGeometry::load(path)` returns `None` for a file containing `{"width":10.0,"height":10.0}` and `Some` for a sane one — which requires making the path injectable (mirroring `personal_dict`'s `Option<&Path>` pattern) or extracting a `fn validate(g) -> Option<Self>` that `load` calls and the test exercises.

### AS-2 — SpellService tests run against the real user profile
- **File:** `loki-app-shell/src/spell/service_tests.rs:7-14, 36-43, 64-70`
- **Tests:** `bootstrap_enables_bundled_english`, `catalog_resolution_and_offline_availability`, `activating_missing_language_errors`, `snapshot_follows_enabled_flag`, `check_reports_misspelling_ranges`
- **Category:** B — **HIGH**
- **Defect:** These call `SpellService::bootstrap()`, which loads the developer's real personal dictionary (`app_data::suite_dir()/personal-dictionary.json`) and points the store at the real dictionaries dir, so several assertions depend on machine state rather than on the code.
- **Evidence:** `bootstrap()` → `bootstrap_with_personal_dict(super::personal_dict::default_path())` (service.rs:55-57) replays real persisted words into the checker; `assert!(!svc.is_correct("teh"))` fails if the user ever added "teh"; `assert!(!svc.is_available_offline("fr"))` and `activate_language("fr").is_err()` fail if French is installed in the real store. The file's own `add_and_ignore_word...` test knows this hazard — it uses `bootstrap_with_personal_dict(None)` with a comment explaining why — but five sibling tests don't.
- **Fixed test would assert:** the same behaviours via `bootstrap_with_personal_dict(None)` plus an injectable store root (temp dir), so no assertion can be flipped by the machine's profile.

### AS-3 — recent-documents round-trip test reads the real user data dir
- **File:** `loki-app-shell/src/recent_documents.rs:231-241`
- **Test:** `entries_round_trip_through_json_without_the_file_name`
- **Category:** B (minor) — **HIGH**
- **Defect:** Calls `RecentDocuments::load("AppThere/Loki/recent.json")` — the developer's live recent list — purely to obtain a value whose `entries` it immediately overwrites; the load path contributes nothing to the assertion but couples the test to the real profile.
- **Evidence:** `let mut docs = RecentDocuments::load("AppThere/Loki/recent.json"); docs.entries = vec![entry("a")];` — only the `#[serde(skip)]` behaviour is then asserted.
- **Fixed test would assert:** the same serde properties on `RecentDocuments::default()` (the `recent_file` field defaults to `""` and is equally skipped); separately, an injectable-path save→load disk round-trip is untested anywhere (see AS-4).

### AS-4 — persistence save/load paths untested across the crate
- **Files:** `loki-app-shell/src/window_geometry.rs` (`load`/`save`/`save_debounced`), `recent_documents.rs` (`load`/`save`), `display_calibration.rs` (`load`/`save`), `document_defaults.rs` (`load`/`load_raw`/`save`)
- **Category:** C — **HIGH**
- **Defect:** No test in the crate exercises any real save→load round trip; all four modules resolve their path from `dirs::data_dir()` internally, so their silent-failure branches (`None` data dir, unreadable file, corrupt JSON at the *load* call site) are never executed. `display_calibration_tests.rs` documents this as deliberate ("`load`/`save` … are deliberately untested here"), but the consequence is that only `personal_dict` (which made its path injectable) has its disk path tested.
- **Fixed test would assert:** with an injected temp path: save→load returns the saved value; corrupt file → default; validation-on-load actually drops implausible values through `DocumentDefaults::load()` itself (currently `is_plausible` is tested directly but the `load()` plumbing at lines 132-145 is not).

### AS-5 — `open_or_switch` on an existing tab: title-preservation not asserted
- **File:** `loki-app-shell/src/tabs_tests.rs:29-35`
- **Test:** `open_existing_tab_switches_without_appending`
- **Category:** C (minor) — **HIGH**
- **Defect:** Passes `"ignored".into()` as the title but never asserts the existing tab's title survived — the one behaviour that name implies beyond the length check.
- **Evidence:** asserts `tabs.len() == 3` and `active == 2` only.
- **Fixed test would assert:** `tabs[1].title == "b"` after the call.

### AS-6 — `parse_new_doc_source` boundary inputs unpinned
- **File:** `loki-app-shell/src/untitled.rs:69-82` (tests 84-134)
- **Category:** C (minor) — **MEDIUM**
- **Defect:** `parse_new_doc_source("untitled-")` (empty counter) returns `Some(Blank)`, and a marker with no digits ("untitled--tpl-x") parses; neither behaviour is pinned, so a rewrite could silently change what counts as an untitled path.
- **Fixed test would assert:** the chosen behaviour for a missing counter and for a marker directly after the prefix.

---

## loki-spell

### SP-1 — `install_dictionary` `NoSource` path untested
- **File:** `loki-spell/src/fetch.rs:55-58`; tests `fetch_tests.rs`
- **Category:** C — **HIGH**
- **Defect:** The `entry.source == None → SpellError::NoSource` branch has no test; every fixture builds a sourced entry.
- **Evidence:** `let source = entry.source.as_ref().ok_or_else(|| SpellError::NoSource(...))?;` — no test constructs `source: None` through `install_dictionary` (store_tests' `test_entry` has `source: None` but never reaches fetch).
- **Fixed test would assert:** `install_dictionary(store, &sourceless_entry, &fetcher, Consent::Granted)` is `Err(SpellError::NoSource(_))` and nothing installed.

### SP-2 — consent gate ordering not discriminated
- **File:** `loki-spell/src/fetch_tests.rs:87-97`
- **Test:** `copyleft_blocked_without_consent`
- **Category:** C — **MEDIUM**
- **Defect:** The doc contract says the consent gate runs *before* any fetch, but the test only asserts the error and non-install; a mutation moving the gate after the download (leaking bytes to the network before refusal) passes.
- **Fixed test would assert:** a counting `MapFetcher` records zero `fetch` calls on a refused install.

### SP-3 — store error tails untested
- **File:** `loki-spell/src/store.rs:104-156`; tests `store_tests.rs`
- **Category:** C — **MEDIUM**
- **Defect:** Three documented behaviours have no test: `load` on non-UTF-8 file → `Io`; `installed()` skipping a subdirectory with corrupt `meta.json` rather than failing the scan; `remove` on a not-installed tag being an `Ok` no-op.
- **Fixed test would assert:** each directly (write invalid bytes/corrupt meta into the temp root and observe).

### SP-4 — checker: clean-input polarity untested
- **File:** `loki-spell/src/checker.rs:115-119, 166-175`; tests `checker_tests.rs`
- **Category:** C (minor) — **MEDIUM**
- **Defect:** `suggest` on an already-correct word (documented to return empty) and `check_text` over fully-correct text (must return no misspellings) are never asserted — the empty-output polarity of both APIs is one-sided.
- **Fixed test would assert:** `c.check_text("hello world").is_empty()` and pin the documented `suggest("hello")` behaviour.

*(Otherwise this crate's tests are strong: fetch integrity/consent both polarities, tokenizer boundaries incl. unicode/curly apostrophes/trailing connectors, catalog policy rejection, locale chain with empties.)*

---

## loki-spreadsheet

### SS-1 — lone-cell-reference-as-range branch has no discriminating test
- **File:** `loki-spreadsheet/src/routes/editor/formula/eval.rs:270-275`; tests `formula_tests.rs`
- **Category:** C — **HIGH**
- **Defect:** The deliberate branch that turns a lone cell reference argument into a 1-cell *range* (so empty cells are excluded from `COUNT`/`AVERAGE`) can be deleted with no test failing — the only lone-ref-in-function test (`SUM(A1:A3, B1, 100)`) is value-equivalent under scalar treatment.
- **Evidence:**
  ```rust
  // eval.rs: "A lone cell reference: a 1-cell range so empty cells
  // are excluded from COUNT/AVERAGE."
  ```
  Mutating this to `Arg::Scalar(self.resolve(r1, c1)?.unwrap_or(0.0))` changes `COUNT(B9)` from 0 to 1 and `AVERAGE(B9)` from `#DIV/0!` to 0 — no test covers either.
- **Fixed test would assert:** `COUNT(B9)` = 0 and `AVERAGE(B9)` = `#DIV/0!` for an empty `B9`; `COUNT(A1)` = 1 for a populated cell.

### SS-2 — UDF "display" tests duplicate the display trimming in the harness
- **File:** `loki-spreadsheet/src/routes/editor/formula_tests.rs:44-58`
- **Tests:** `udf_computes_a_numeric_result`, `builtins_are_not_shadowed_by_the_resolver` (via `eval_udf`)
- **Category:** B — **HIGH**
- **Defect:** The `eval_udf` helper reimplements integer trimming (`// Mirror the display path's integer trimming`) instead of going through `evaluate_cell`/`format_number`, so a regression in the production display formatting of UDF results is invisible to these tests.
- **Evidence:** helper: `if n == n.trunc() { format!("{}", n as i64) } ...` — a second copy of the `format_number` fact (production formats via `format_number` at mod.rs:121). The numeric substance is still tested; the display claim is not.
- **Fixed test would assert:** UDF-in-cell display through `evaluate_cell` on a workbook whose cell formula is `Doubler(A1)` (as `floating_point_noise_is_trimmed_in_display` already does for arithmetic).

### SS-3 — text-UDF in arithmetic context (`#VALUE!` arm) untested
- **File:** `loki-spreadsheet/src/routes/editor/formula/eval.rs:171-174`
- **Category:** C — **HIGH**
- **Defect:** The `CellValue::Text(_) => Err(FormulaError::Value)` arm (a text UDF used numerically, e.g. `Greeting()+1`) is reachable and unexercised; likewise `value_to_cell`'s `Bool → "TRUE"/"FALSE"`, `Empty/Null → ""`, and `Array/Object → #VALUE!` arms (funcs.rs:81-93) have no tests.
- **Fixed test would assert:** `eval_udf("Greeting()+1", ...) == "#VALUE!"`; a bool-returning UDF displays `TRUE`.

### SS-4 — misc evaluator branches without coverage
- **File:** `loki-spreadsheet/src/routes/editor/formula/` ; tests `formula_tests.rs`
- **Category:** C — **MEDIUM**
- **Defect:** No test for: non-finite result → `#NUM!` (mod.rs:151-153, e.g. `1e308*10`); leading-`=` stripping in `evaluate_formula` (all tests pass bare expressions); reversed ranges (`SUM(A3:A1)`, normalized by `collect_range`'s min/max); IF wrong arity on the *falsy* path (`IF(0,1,2,3)` — only truthy-path arity errors are tested); empty formula → 0.
- **Fixed test would assert:** each in one compact test; the `#NUM!` and `=`-prefix cases matter most (both are documented behaviours).

*(cell_ref.rs tests are exemplary: round-trips, both out-of-range boundaries, malformed refs, case/trim.)*

---

## loki-primitives

### PR-1 — `Affine2::rotation` only tested at angle 0
- **File:** `loki-primitives/src/geometry/transform.rs:148-155`
- **Test:** `test_rotation`
- **Category:** C — **HIGH**
- **Defect:** `rotation(0.0)` degenerates to the identity (`cos=1, sin=0`), so swapping `sin`/`-sin` (handedness), transposing the b/c coefficients, or any sign error in `rotation` passes the whole suite.
- **Evidence:** `let t = Affine2::rotation(0.0); ... assert!(t.is_identity());` — the only rotation test.
- **Fixed test would assert:** `rotation(FRAC_PI_2).transform_point(1.0, 0.0) ≈ (0.0, 1.0)` (pinning the direction convention), and `rotation(π/2) ∘ rotation(-π/2)` ≈ identity.

### PR-2 — `Affine2::then` composition order unpinned
- **File:** `loki-primitives/src/geometry/transform.rs:60-74`; test `test_inverse` (158-168)
- **Category:** C — **HIGH**
- **Defect:** The only test through `then` is an inverse round-trip, which any invertible transform satisfies regardless of operand order — mutating `then` to compose in the reverse order ("other then self") fails no test.
- **Evidence:** `let t = scale.then(translation); ... inv.transform_point(t.transform_point(p))` restores `p` for *either* composition order.
- **Fixed test would assert:** `scale(2,2).then(translation(4,5)).transform_point(1,1) == (6.0, 7.0)` (scale applied first) — a point the reversed order maps to `(10.0, 12.0)`.
- Also untested: `transform_size` (never called by any test), `uniform_scale`, `Rect::outset`.

### PR-3 — `from_hex` accepts garbage alpha digits; the laxity has no test either way
- **File:** `loki-primitives/src/color/document.rs:77-104`; tests 127-161
- **Category:** C — **HIGH** (production laxity surfaced by the gap)
- **Defect:** For an 8-char payload only the first 6 hex digits are parsed; `"#123456ZZ"` is accepted as a valid color because the discarded alpha bytes are never validated. No test pins accept-or-reject, so the behaviour is accidental.
- **Evidence:** length check `hex.len() != 6 && hex.len() != 8` then parses only `hex[0..6]`; the alpha test case uses valid digits (`"#FF8000CC"`).
- **Fixed test would assert:** `from_hex("#123456ZZ")` is an error (after adding validation of the alpha pair — the honest root-cause fix), or explicitly pin the discard-without-validation contract if that is intended.

*(measurement/length tests are exemplary — resolution chain both polarities at every rung, format/parse round-trip drift bound, rejection polarity list. `rect.rs` and `display_*` in app-shell likewise.)*

---

## loki-sheet-model

### SM-1 — "deterministic" same-cell merge test proves convergence, not determinism
- **File:** `loki-sheet-model/tests/loro_concurrency_tests.rs:121-137` (same pattern in presentation-model tests:117-139)
- **Test:** `concurrent_edit_of_same_cell_is_deterministic`
- **Category:** B (minor) — **MEDIUM**
- **Defect:** The name and message claim one writer wins *deterministically*; the body asserts the replicas agree and the value is one of the two — a merge that picked a random-but-synced winner per run would pass.
- **Evidence:** `assert!(matches!(v.as_deref(), Some("value-A") | Some("value-B")))` after `a.get_deep_value() == b.get_deep_value()`.
- **Fixed test would assert:** run the same concurrent edit twice from identical seeds/peer-ids and assert the same winner both times (or rename to `..._converges_to_one_value`, which is what is actually established).

---

## loki-presentation-model

### PM-1 — suite doc claims slide-snapshot LWW is pinned; no test edits a slide snapshot concurrently
- **File:** `loki-presentation-model/tests/loro_concurrency_tests.rs:14-17` (doc) vs tests
- **Category:** B — **HIGH**
- **Defect:** The module doc says "concurrent edits to the *same* slide snapshot string converge to a single deterministic value … the test exists to make a future fine-grained bridge a visible, intentional behaviour change" — but the only same-key test targets metadata `title`; no test writes the `drawing`/`placeholders` snapshot key from two replicas, so the documented MVP limitation is unguarded.
- **Fixed test would assert:** two replicas write different `drawing` JSON to the same slide map key, sync, and one whole snapshot wins (no field blend) — the exact behaviour a fine-grained bridge would intentionally change.

### PM-2 — bridge error/skip paths untested
- **File:** `loki-presentation-model/src/loro_bridge.rs:117-124, 107-114`
- **Category:** C — **MEDIUM**
- **Defect:** `loro_to_presentation` with a corrupt `drawing` JSON string (→ `BridgeError::Json`), a missing `id` (→ synthesized `slide{i}`), and a non-map slide-list entry (→ `continue`) are all unexercised; only clean round-trips are tested.
- **Fixed test would assert:** a doc with `drawing = "not json"` errors as `Json`; a slide map without `id` restores with the synthesized id.

---

## loki-presentation

### PP-1 — `add_bullet`'s create-missing-body arm never executes
- **File:** `loki-presentation/src/routes/editor/edit.rs:72-84`; test `add_bullet_appends_paragraph` (162-171)
- **Category:** C — **HIGH**
- **Defect:** Every test reaches `add_bullet` via `add_slide`, which pre-installs a `"body"` placeholder, so the `None => { push shape; add_placeholder }` arm (creating the body shape on a slide that lacks one) is dead in the test suite — deleting it fails nothing.
- **Fixed test would assert:** `add_bullet` on a slide built with bare `Slide::new` (no placeholders) creates the body shape, registers the `Body` placeholder, and the bullet is visible in `slide_views`.

### PP-2 — mutation no-op guards never false/true-side tested
- **File:** `loki-presentation/src/routes/editor/edit.rs:24-31, 59-63`
- **Category:** C — **MEDIUM**
- **Defect:** `set_shape_text` with an out-of-range slide index, a missing shape id, or a non-geometry shape (three early returns), and `delete_slide` with `index >= len` while multiple slides exist, are never exercised — the "no-op if missing" contract in the doc comment is untested.
- **Fixed test would assert:** each no-op leaves the model unchanged (e.g. `set_shape_text(&mut p, 9, ...)` then `p` equals its prior value).

### PP-3 — slide_view: subtitle, CenteredTitle fallback, and color derivation untested
- **File:** `loki-presentation/src/routes/editor/slide_view.rs:60-67, 86-87, 133-156`
- **Category:** C — **MEDIUM**
- **Defect:** `subtitle`, the `CenteredTitle` → title fallback, `bg_css` from a solid fill (vs. the `#FFFFFF` default), and `first_text_color` (vs. `#1A1A1A` default) have no tests; only the Title/Body/loose-shape flattening is covered.
- **Fixed test would assert:** a slide with `CenteredTitle` yields a title view; a solid-fill background yields its hex.

---

## loki-fonts

### FT-1 — "same set" check asserts only a blob count
- **File:** `loki-fonts/src/lib.rs:210-219`
- **Test:** `every_named_family_has_bundled_faces`
- **Category:** B — **HIGH**
- **Defect:** The doc comment claims the name list and blob list "describe the same set… the check is mechanical", but the assertion is arithmetic on counts (`2 + (n−1)*4`); replacing all four Gelasio blobs with duplicate Tinos files (or renaming a family while keeping its blob count) passes.
- **Evidence:** `assert_eq!(fallback_font_blobs().len(), expected, ...)` — cardinality, not membership.
- **Fixed test would assert:** parse each blob's name table (e.g. with `read-fonts`/`ttf-parser` as a dev-dependency) and assert the set of family names in the blobs equals `bundled_families()` names — or at minimum assert each blob's bytes start with a valid sfnt tag and pair the per-family counts explicitly.

### FT-2 — `fallback_font_blobs_embedded_on_all_targets` is a near-tautology on the test target
- **File:** `loki-fonts/src/lib.rs:247-253`
- **Category:** A (minor) — **HIGH**
- **Defect:** `FACES` is a compile-time static of 26 `include_bytes!` entries; on the host target the non-empty assertion cannot fail unless someone cfg-gates the list — which is the one regression it guards, but only on targets where tests run (the claim "on every platform" is untestable here).
- **Fixed test would assert:** keep it as the cfg-gating canary but drop the "every platform" implication, or additionally assert `blobs.iter().all(|b| b.len() > 1024)` so a truncated asset also fails.

---

## loki-i18n

### I18-1 — primary-locale-wins lookup order is untestable (and untested) with only en-US embedded
- **File:** `loki-i18n/src/loader.rs:71-81` (lookup order), `i18n/` (assets: `en-US` only)
- **Category:** C — **HIGH**
- **Defect:** `get`'s documented order (primary bundle → en-US fallback → raw key) is only ever exercised with an *empty* primary bundle (every non-en-US load) or with primary == fallback content (en-US): no test can show a primary translation shadowing the fallback, so a regression that always consulted the fallback first would be invisible. Every existing test resolves to the same `en-US` value by construction.
- **Fixed test would assert:** embed a tiny test-only locale (or construct a `LokiBundle` from an in-memory `FluentResource` in tests) where `shell-app-name` differs from en-US, and assert the primary's value wins while a key missing from the primary falls back.

### I18-2 — argument interpolation never tested
- **File:** `loki-i18n/src/loader.rs:71` (`args` parameter), `format_from` (109-115)
- **Category:** C — **MEDIUM**
- **Defect:** All six tests pass `None` for args; `format_pattern` with `FluentArgs` (the path the `fl!` macro uses for parameterized strings) has zero coverage, including the errors-vector behaviour on a missing argument.
- **Fixed test would assert:** a known parameterized key from the embedded `.ftl` renders with a supplied arg (and pins the missing-arg fallback rendering).

*(The existing tests are otherwise good: they assert the resolved value `"Loki Text"`, not merely "resolves" — the en-US-posix regression test is exactly right.)*

---

## loki-templates

No findings. `every_template_id_imports` alone would be weak (is_some only), but the
suite backs it with value-level assertions per template (fonts, spacing, indents,
next-style chains, the single direct page break with its position and style ref,
and the bundled-faces-only sweep that cross-checks against `loki_fonts`). The
blank-template test asserts both polarities (not in gallery, no asset).

---

## Per-crate stats

| Crate | Test files read | Tests examined | A | B | C |
|---|---|---|---|---|---|
| loki-app-shell | 9 | 55 | 1 | 2 | 3 |
| loki-spell | 7 | 40 | 0 | 0 | 4 |
| loki-spreadsheet | 2 | 35 | 0 | 1 | 3 |
| loki-primitives | 6 | 31 | 0 | 0 | 3 |
| loki-sheet-model | 2 | 19 | 0 | 1 | 0 |
| loki-presentation-model | 4 | 13 | 0 | 1 | 1 |
| loki-presentation | 2 | 7 | 0 | 0 | 3 |
| loki-fonts | 1 | 6 | 1 | 1 | 0 |
| loki-i18n | 1 | 6 | 0 | 0 | 2 |
| loki-templates | 1 | 9 | 0 | 0 | 0 |
| **Total** | **35** | **221** | **2** | **6** | **19** |

## Healthy areas

- **loki-app-shell `display_density` / `display_calibration` / `document_defaults` / `measurement`** are model test suites: every guard tested in both polarities, boundaries pinned on both sides (`49.0`/`51.0`, `36.0`/`14400.0`), NaN/∞ cases, and explicit "the polarity: …" assertions that prevent a test passing because nothing loads.
- **loki-spell tokenizer/fetch/catalog** cover unicode, curly apostrophes, trailing connectors, tamper detection, and the consent gate in both directions.
- **loki-spreadsheet cell_ref** round-trips labels through both converters and tests one-past-max on both axes.
- **loki-templates** asserts document *content* (values, chains, block positions) rather than mere import success.
- The two Loro concurrency suites correctly separate structural merges from register semantics and test snapshot/idempotency; their only weakness is the naming/doc overclaim noted in SM-1/PM-1.
- The formula evaluator's expected values are hand-derivable (1+2*3, definitional conversions) — the "expected computed by the engine under test" anti-pattern was checked for and **not found**; the closest instance is the harness-side display trimming in SS-2.
