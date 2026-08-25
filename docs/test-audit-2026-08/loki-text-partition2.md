# loki-text — partition 2 findings (dom_reflow, editing, keydown/next-style; 15 files, 109 tests)

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

Partition stats: 15 files read, 109 tests examined, A=1 B=2 C=4.

Healthy areas: this partition is largely exemplary. `content_tests.rs`, `style_tests.rs`, `image_tests.rs`, `editor_insert_tests.rs`, `editor_format_range_tests.rs`, `editor_next_style_tests.rs`, and the keydown/newline/enter/lists suites consistently check preconditions before asserting, run the real resolver instead of hand-building fixtures, and include explicit inversions (e.g. `a_run_that_differs_keeps_its_own_span`, `kerning_is_off_unless_the_run_asks_for_it`, `tab_guard_refuses_plain_paragraphs_and_boundaries`); `editor_layout_task.rs`'s inline tests cover both the publish-lands and publish-yields-to-racing-edit sides of the generation guard.
