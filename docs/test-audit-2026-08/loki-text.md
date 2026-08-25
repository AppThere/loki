# loki-text — test-efficacy audit

Crate: `loki-text`. Method: every test file enumerated first; for each suspicious test the production
code it calls was read before claiming a finding; mutation thought-experiment applied ("would this
test fail if the production function were stubbed or its guard inverted?"). No repo files modified;
no cargo runs (static analysis only).

**Coverage status:** the audit was partitioned five ways. Partitions 2 (dom_reflow / editing keydown
/ next-style, 15 files) and 5 (link/meta/page/publish/span/table/print dialogs, 14 files) are
complete and reported below. Partitions 1 (src root + `src/editing/` + home/startup routes +
`tests/wgpu_surface_integration.rs`, ~20 files), 3 (`editor_macro_*`, `editor_ribbon_*`, colors,
zoom, spell, responsive, style-target, font-family, dialog_walk, ~23 files) and 4
(`editor_style_editor/*`, `style_dialog/*`, `style_*_inspector`, `style_impact`, ~18 files) stalled
on a usage-limit outage and are being resumed separately; their findings will be merged into this
file by the coordinator. Findings and stats below therefore cover partitions 2 + 5 (29 files, 220
tests examined).

---

## Findings — partition 2 (dom_reflow, keydown/editing, next-style)

### `src/routes/editor/dom_reflow/list_tests.rs:68` — `nesting_accumulates_the_indent` — Category A — HIGH
**Defect:** The test performs pure arithmetic on values it computed itself and never calls the production code that actually accumulates the indent.
**Evidence:** `let outer = 0.0 + NESTED_INDENT_PT; let inner = outer + NESTED_INDENT_PT; assert_eq!(inner, 2.0 * NESTED_INDENT_PT); assert!(inner > outer)` — a tautology over a constant. The real accumulation is `list_el`'s `indent + NESTED_INDENT_PT` plus the nested-list recursion arm in `list.rs:67/121-131`, neither of which is exercised; deleting the accumulation from `list_el` fails no test.
**Fix:** Drive `list_el`/`item_el` (or extract their indent computation into a testable function) with a nested list and assert the synthesized inner paragraph's `indent_start` is `2 × NESTED_INDENT_PT` while the outer one's is `1 ×`.

### `src/routes/editor/dom_reflow/oversized_tests.rs:68` — `an_unfittable_element_stays_in_the_fitted_box` — Category B — HIGH
**Defect:** The test exercises a local reimplementation of the component's guard, not the component: mutating `AtOversized`'s own `inner_css(fittable && expanded())` (oversized.rs:97) to `inner_css(expanded())` fails no test.
**Evidence:** `let inner_for = |fittable: bool, expanded: bool| inner_css(fittable && expanded);` — the guard under test is restated inside the test (the comment admits "the component's own rendering is not reachable from a unit test").
**Fix:** Extract the `fittable && expanded` decision into a named production function the component calls, and test that function — so the guard has one derivation and its inversion actually bites.

### `src/routes/editor/dom_reflow/table_tests.rs:54` — `a_proportional_column_is_a_percentage` — Category B — HIGH
**Defect:** The assertion checks only that the output contains `%` and no `pt`, so the share-to-percent arithmetic (`share * 100.0`, table.rs:107) is unverified — mutating the multiplier to `* 10.0` or `* 1.0` passes.
**Evidence:** `let css = col_css(&ColWidth::Proportional(0.25)); assert!(css.contains('%')); assert!(!css.contains("pt"))` — the value 25 is never asserted.
**Fix:** `assert_eq!(col_css(&ColWidth::Proportional(0.25)), "width: 25%;")`.

### `src/routes/editor/dom_reflow/list.rs:87` — `item_el` / `list_el` rendering rules — Category C — MEDIUM
**Defect:** The load-bearing per-item decisions — marker only on a first `StyledPara`, nested list recursed *without* the padding wrapper (the measured 24 pt double-indent defect), other blocks getting `padding-inline-start` — have no unit test at all; the sibling tests pin only the `loki_layout` helpers.
**Evidence:** All four match arms of `item_el` (list.rs:97–142) build rsx directly; `list_tests.rs` never constructs a list with a nested list, a leading table, or a second paragraph.
**Fix:** Extract the per-block decision (marker? indent-as-padding? recurse-with-which-indent?) into a pure function returning an enum, and test each arm including the no-double-indent case.

### `src/routes/editor/editor_keydown_text_tests.rs:64` — `delete_selection_in_doc` suite — Category C — MEDIUM
**Defect:** Every call passes `deletion: None`, so the track-changes path (a `Some(RevisionMark)` threaded to `tracked_delete_selection_at`, editor_keydown_text.rs:93-98) is never exercised in this crate.
**Evidence:** `delete_selection_in_doc(&loro, &cs, None)` in all five delete tests; no test asserts that with a mark the text is *marked* deleted rather than removed.
**Fix:** One test with a `RevisionMark` asserting the selected text survives in the rebuilt document carrying the deletion revision, and the cursor still collapses to the range start.

### `src/routes/editor/editor_next_style.rs:98` — `heading_level_of` bounds — Category C — HIGH
**Defect:** The `(1..=6).contains(&level)` guard has no failing-side coverage at the numeric boundary: no test uses a next style of `Heading0` or `Heading7`, so widening the range to `1..=9` fails nothing.
**Evidence:** `editor_next_style_tests.rs` covers `Heading2`, `"Heading 3"`, and the non-numeric `HeadingBanner`, but never an out-of-range numeric level.
**Fix:** `next_block_for_split` with `next_style_id = "Heading7"` must yield `NextBlock::Styled("Heading7")`, not `NextBlock::Heading(7)`.

### `src/routes/editor/dom_reflow/editing_tests.rs:116` — `resolve_click_position` below-document case — Category C — MEDIUM
**Defect:** The documented no-op for "a click below the last paragraph" is tested only via an *empty* layout; a y past `total_height` on a populated layout (the actual production case) is never exercised, so whether `hit_test` returns `None` or clamps to the last paragraph is unpinned.
**Evidence:** `empty_layout_resolves_to_none` uses `paragraphs: vec![]`; no test clicks at `pt_to_px(total_height + 50.0)` on `two_para_continuous()`.
**Fix:** Assert the behaviour for a click well below the second paragraph (whichever is intended — `None` or clamp-to-end — pin it).

---

## Findings — partition 5 (link/meta/page/publish/span/table/print dialogs)

### `src/routes/editor/publish_dialog/fonts_tests.rs:69` — `a_face_used_twice_is_listed_once` — Category C — HIGH
**Defect:** The case-insensitive dedup that `insert_name`'s own doc comment records as a fixed bug ("keying on the raw name listed a case-varied import twice") has no test — both occurrences in the test spell the family identically, so removing the `.to_lowercase()` key (fonts.rs:88-94) fails nothing.
**Evidence:** `Block::Para(vec![run_with_face("Tinos")]), Block::Para(vec![run_with_face("Tinos")])` → `assert_eq!(used_faces(&d).len(), 1)`. Production: `names.entry(trimmed.to_lowercase()).or_insert_with(|| trimmed.to_string())` — the lowercase key and first-spelling-wins display are both unpinned.
**Fix:** A doc using `"TINOS"` and `"tinos"` asserts `used_faces(...).len() == 1` and that the displayed `name` is the first spelling seen.

### `src/routes/editor/publish_dialog/fonts_tests.rs:111` — `nested_runs_are_collected` — Category B — HIGH
**Defect:** The doc comment claims "faces inside notes, tables and links still reach the file", but the body exercises only a link — the note and table walks (delegated to `dialog_walk`) are untested from this call site.
**Evidence:** The only fixture is `Inline::Link(..., vec![run_with_face("Carlito")], ...)`; no `Inline::Note` or `Block::Table` appears in the file.
**Fix:** Extend the fixture with a face inside a footnote body and one inside a table head-row cell, asserting all three families are listed.

### `src/routes/editor/publish_dialog/preflight_tests.rs:205` — `table_and_image_checks_are_omitted_when_there_are_none` — Category B — HIGH
**Defect:** The test name claims both table and image checks are omitted, but the assertion greps findings only for `"table"` — an image check wrongly emitted on an image-less document passes.
**Evidence:** `assert!(!finding.message.to_lowercase().contains("table"), ...)` is the sole predicate in the loop; no `contains("image")` (or `alt`) check exists.
**Fix:** Assert the same absence for image/alt-text findings in the same loop, or assert the exact expected finding set for `publishable()`.

### `src/routes/editor/print_dialog_tests.rs:11` — `options_map_from_field_strings` — Category C — HIGH
**Defect:** The `copies` boundary `"0"` is untested, and production has no floor — `copies.trim().parse().unwrap_or(1)` (print_dialog_support.rs:26) forwards `copies: 0` to the IPP job, which the test suite would never catch.
**Evidence:** Tests cover `"3"`, `""`, and `"many"` (both → 1 via parse failure), but `"0"` parses successfully and bypasses `unwrap_or(1)`. No clamp like `.max(1)` exists in `build_ipp_options`.
**Fix:** `assert_eq!(build_ipp_options("0", "", false, "T".into()).copies, 1)` — which requires adding the missing floor to production (the real defect this gap hides).

### `src/routes/editor/link_dialog/target_tests.rs:62` — `slugify` / `collapse_dashes` coverage — Category C — HIGH
**Defect:** `collapse_dashes` exists solely as a documented regression fix ("`replace(\"--\", \"-\")` is a single non-overlapping pass, so three or more in a row survived it" — target.rs:178-194), yet no test slugs a label containing any punctuation, so reverting the fix — or deleting `trim_matches('-')` or the `heading-{ordinal}` empty-base fallback — fails nothing.
**Evidence:** The only headings slugified without an explicit id are `"Introduction"`/`"Introduction"` (pure ASCII letters); the label `"One · The harbour"` in `headings_and_bookmarks_are_listed_in_document_order` carries an explicit id `"one"`, so `slugify` never sees it.
**Fix:** Assert `document_targets` on an id-less heading `"One · The harbour"` yields anchor `"one-the-harbour-0"` (no `--` runs, no leading/trailing dash), and an id-less all-punctuation heading yields `"heading-N"`.

### `src/routes/editor/link_dialog/validate_tests.rs:10` — `LinkKind::File` valid arm never exercised — Category C — HIGH
**Defect:** `validate(LinkKind::File, <non-empty>)` is called by no test, so the arm returning `Valid` for any non-blank file path (validate.rs:54) could be inverted to `Invalid` — making file links un-insertable — and the suite still passes.
**Evidence:** The only File-kind calls are the whitespace loop in `an_empty_address_is_neither_valid_nor_complained_about` (returns `Empty` before the match) and `to_url(LinkKind::File, ...)`, which never consults `validate`.
**Fix:** `assert!(validate(LinkKind::File, "/home/a.odt").can_insert())`.

### `src/routes/editor/meta_dialog/stats_tests.rs:111` — `tables_and_images_inside_notes_are_still_counted` — Category C — HIGH
**Defect:** The deliberate exclusion of note-body characters/paragraphs from the outer counts (stats.rs:122-131 walks the note into a discarded `inner` DocStats, adding back only tables and images, to keep characters consistent with `count_words`) is unasserted — a mutation that also adds `inner.characters` fails no test.
**Evidence:** The test asserts `notes == 1`, `tables == 1`, `images == 1` but never asserts `characters` or `paragraphs` for a document whose only text lives in a note body.
**Fix:** Add `assert_eq!(stats.characters, 0)` and `assert_eq!(stats.paragraphs, 1)` (the host paragraph only) to the same fixture; separately, `CodeBlock`/`LineBlock`/`DefinitionList` arms of `walk_blocks` (stats.rs:69-92) have no test at all.

---

## Findings — partitions 1, 3, 4 (in sibling files)

These partitions completed separately; their findings are in
[`loki-text-partition1.md`](loki-text-partition1.md),
[`loki-text-partition3.md`](loki-text-partition3.md), and
[`loki-text-partition4.md`](loki-text-partition4.md)
(partition 2's are duplicated in [`loki-text-partition2.md`](loki-text-partition2.md)).
Files covered: partition 1 — `src/device_probe_tests.rs`, `src/texture_budget_tests.rs`, inline
tests in `src/utils.rs` / `src/window_state.rs` / `src/startup_files.rs` / `src/editing/touch.rs` /
`src/editing/reflow_nav.rs`, all `src/editing/*_tests.rs` (incl. `page_locate_characterisation_tests.rs`),
`src/routes/home_templates_tests.rs`, `src/routes/startup_open_tests.rs`,
`tests/wgpu_surface_integration.rs`. Partition 3 — `editor_macro_*_tests.rs` (4),
`editor_ribbon_*_tests.rs` (5), `editor_defaults`, `editor_doc_colors`, `editor_highlight_color`,
`editor_text_color`, `editor_responsive`, `editor_zoom`, `editor_wheel_zoom`, `editor_seed_publish`,
`editor_spell_place`, `editor_spell_rows`, `editor_font_warning`, `editor_style_target`,
`font_family_list`, `dialog_walk` tests. Partition 4 — `editor_style_editor/*_tests.rs` (6) +
`draft_table.rs` inline, `style_dialog/*_tests.rs` (6), `style_impact`, `style_inspector`,
`style_char_inspector`, `style_list_inspector`, `style_page_inspector` tests.

---

## Stats (all five partitions)

| Partition | Test files read | Tests examined | A | B | C |
|---|---|---|---|---|---|
| 1 (src root, `src/editing/`, home/startup, wgpu integration) | 20 | 126 | 3 | 8 | 7 |
| 2 (dom_reflow / keydown / next-style) | 15 | 109 | 1 | 2 | 4 |
| 3 (macro bridge/apply/run, ribbon, colors, zoom, spell) | 23 | 154 | 2 | 6 | 2 |
| 4 (style editor / style dialog / inspectors / page presets) | 18 | 126 | 1 | 0 | 6 |
| 5 (dialogs: link/meta/page/publish/span/table/print) | 14 | 111 | 0 | 2 | 5 |
| **Total** | **90** | **626** | **7** | **18** | **24** |

Findings for partitions 1, 3, and 4 are in the sibling partition files listed above; the
detailed text below covers partitions 2 and 5.

## Healthy areas

The audited portion of this crate is well above average. Partition 2: `content_tests.rs`,
`style_tests.rs`, `image_tests.rs`, `editor_insert_tests.rs`, `editor_format_range_tests.rs`, and
the keydown/newline/enter/lists suites consistently check preconditions before asserting, run the
real resolver instead of hand-building fixtures, and include explicit inversions (e.g.
`a_run_that_differs_keeps_its_own_span`, `kerning_is_off_unless_the_run_asks_for_it`,
`tab_guard_refuses_plain_paragraphs_and_boundaries`); `editor_layout_task.rs`'s inline tests cover
both the publish-lands and publish-yields-to-racing-edit sides of the generation guard. Partition 5:
`page_dialog/tabs_tests.rs` is exemplary — `a_ticked_variant_box_means_the_band_exists` pins a
previously-inverted predicate against its mutation in both directions, and
`each_variant_toggle_touches_only_its_own_band` checks cross-band isolation;
`span_dialog/char_style_tests.rs` pins the diff-guard with a genuinely discriminating
partial-selection fixture and pins `MARK_CHAR_STYLE_ID` *out* of `OWNED_MARKS`; the `SpanMarks`
tests are structured so adding a struct field breaks compilation of the exhaustiveness tests;
`table_dialog/spec_tests.rs` covers both clamp ends, the 1-row-with-header case, and underflow;
`preflight_tests.rs` (aside from the one finding) tests both severity directions
(warnings-don't-block / errors-do) and the header-row walk regression.
