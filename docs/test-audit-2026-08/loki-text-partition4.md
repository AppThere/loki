# loki-text — partition 4 findings (style dialog, inspectors, page presets, draft table; 18 files, 126 tests)

### `src/routes/editor/style_dialog/tab_stops_tests.rs:53` — `clearing_produces_an_empty_local_list` — Category A — HIGH
**Defect:** The fixture passes an already-empty base, so the clear edit has no observable effect and the assertion holds for a stub of `edited_stops` that ignores the closure entirely.
**Evidence:** `let after = edited_stops(&[], |list| list.clear()); assert!(after.is_empty());` — an implementation returning `shown.to_vec()` unchanged (never invoking `edit`) passes. The behaviour the doc comment claims ("an explicit empty local list rather than a fall-through to the parent") is decided in `edit_stops` (`tab_stops = Some(edited_stops(...))`), which no test calls.
**Fix:** Pass a non-empty base and assert the result is empty; separately assert (via `edit_stops` or its equivalent pure seam) that clearing writes `Some(vec![])` into `para_props.tab_stops`, not `None`.

### `src/routes/editor/style_dialog/tab_stops_edit.rs:52` — `add_stop` / `edit_stops` (missing tests) — Category C — HIGH
**Defect:** `add_stop`'s real logic — replace-at-same-position dedupe, unparseable-buffer early return, and buffer clearing — plus `edit_stops`' `Some(...)` write are entirely untested.
**Evidence:** `if let Some(existing) = list.iter_mut().find(|s| (s.position.value() - position.value()).abs() < f64::EPSILON) { existing.position = position; } else { list.push(...) }` — deleting the dedupe branch (always push) or inverting the `parse_points` guard fails no test; the only tested function in the file is `edited_stops`.
**Fix:** Tests that adding a stop at an existing position yields one stop, junk buffer text leaves the list and draft untouched, and a successful add empties `buffers.new_tab_stop`.

### `src/routes/editor/style_impact.rs:19` — `property_is_set` (one-sided coverage) — Category C — HIGH
**Defect:** Nine of the eleven `StyleProperty` arms in the shadowing predicate are never exercised, so a transposed field in any of them (e.g. `IndentStart => pp.indent_end.is_some()`) would fail no test.
**Evidence:** `style_impact_tests.rs` only ever passes `StyleProperty::Alignment` and `StyleProperty::FontSize` to `affected_dependents`; the predicate is a hand-maintained duplicate of `clear_local_property`'s field mapping, fenced only by a comment ("mirrors the property↔field mapping").
**Fix:** For each `StyleProperty`, a dependent overriding exactly that property must drop out of the affected set — a loop over all 11 variants with a per-variant override catches any arm/field transposition.

### `src/routes/editor/style_inspector.rs:224` — `clear_local_property` (one-sided coverage) — Category C — HIGH
**Defect:** Only 4 of the 11 clear arms (Alignment, IndentStart, Bold, FontFamily) are exercised, so a swapped field in the other seven (e.g. `SpaceBefore => pp.space_after = None`) passes the suite.
**Evidence:** `style_inspector_tests.rs` calls `clear_local_property` with `Alignment`, `IndentStart`, `Bold`, `FontFamily` only; the untested arms include the easily-transposable pairs `IndentEnd`/`IndentFirstLine` and `SpaceBefore`/`SpaceAfter`. The `fmt_bool` false→"Off", `fmt_spacing` non-Exact, and `fmt_line_height` `AtLeast` render paths are also never asserted through `paragraph_inspector_rows`.
**Fix:** Loop over every `StyleProperty`: set that property (and only it), clear it, and assert exactly that row went `FormatDefault` while the other ten are unchanged.

### `src/routes/editor/editor_style_editor/draft_table.rs:118` — inline tests (gaps) — Category C — HIGH
**Defect:** The `display_name` normalisation (`name` empty or equal to `id` → `None`), the `col_band_str` field, and the `base: None` fresh-style creation path are untested.
**Evidence:** `style.display_name = if draft.name.is_empty() || draft.name == draft.id { None } else { Some(...) }` — no test sets `draft.name` to `""` or to the id; both tests pass `Some(style)` as base, so `draft_apply_to_table_style(_, None)` never runs; only `row_band_str` is asserted.
**Fix:** Assert a draft named identically to its id applies with `display_name == None`; assert `col_band_str` parses/clears symmetrically to the row test; assert applying onto `None` yields a style with the draft's id and default (empty) conditional map.

### `src/routes/editor/editor_style_editor/page_presets.rs:158` — `is_active` columns arms + widths filter — Category C — HIGH
**Defect:** `is_active` for `Columns(n)` and `ToggleSeparator` is never asserted, and the widths-preservation filter in the `Columns` apply arm (explicit widths kept only when their length matches the new count) is never exercised.
**Evidence:** `PagePreset::Columns(n) => count == n` and `PagePreset::ToggleSeparator => ...is_some_and(|c| c.separator)` have no test caller; `.filter(|w| w.len() == usize::from(n))` — no test constructs a layout with explicit `widths`, so deleting the filter (carrying mismatched widths to a new count) fails nothing.
**Fix:** Assert `is_active(two-col layout, Columns(2))` and its negations; assert a 2-column layout with explicit widths keeps them under `Columns(2)` re-apply but drops them (equal columns) under `Columns(3)`.

### `src/routes/editor/style_char_inspector_tests.rs:49` — missing local-`false` case — Category C — MEDIUM
**Defect:** The character-family inspector never tests a locally-set `false` (e.g. `bold: Some(false)`), the exact "un-bolding a bold parent looks like it did nothing" hazard the paragraph-family suite (`rows_tests.rs:133`) explicitly pins.
**Evidence:** All char-inspector fixtures use `Some(true)` or unset; a resolver that conflated `Some(false)` with `None` (falling through to the parent's `true`) would pass every test in this file.
**Fix:** A child char style with `bold: Some(false)` under a bold parent must produce a `Local` row displaying "Off".

Partition stats: 18 test files read (plus 5 production modules verified: `tab_stops_edit.rs`, `style_impact.rs`, `style_inspector.rs`, `page_presets.rs`, `draft_table.rs`), 126 tests examined, A=1 B=0 C=6.

Healthy areas: this partition is unusually strong overall. `page_margin_fields_tests`, `page_size_picker_tests`, `page_presets_tests` (apply side), `borders_tests`, `draft_tests`, `rows_tests`, `body_tests`, and `style_page_inspector_tests` systematically invert their guards, test both rejection polarities, guard their own fixtures against tautology (e.g. asserting the base differs from the default before asserting preservation), and cover RTL, cycles, and unit-dependence. The findings are concentrated in untested mapping arms and the tab-stop edit layer, not in weak assertions.
