# loki-text — partition 1 findings (device_probe, texture_budget, utils, window_state, editing/*, wgpu integration; 20 files, 126 tests)

### `tests/wgpu_surface_integration.rs:26` — `generation_wraps_without_panic` — Category A — HIGH
**Defect:** The test exercises no production code at all — it asserts that Rust's `u64::wrapping_add` wraps.
**Evidence:** `let max_gen = u64::MAX; assert_eq!(max_gen.wrapping_add(1), 0);` — no `DocumentState` value is even constructed.
**Fix:** Drive the production mutation path (whatever increments `DocumentState::generation`) at `u64::MAX` and assert the state's own field wraps to 0 without panicking.

### `tests/wgpu_surface_integration.rs:44` — `generation_is_monotone_across_document_changes` — Category A — HIGH
**Defect:** The test increments `generation` itself in its own loop and then asserts its own increments were monotone — a pure tautology; no production code performs "document changes."
**Evidence:** `s.generation = s.generation.wrapping_add(1); assert!(s.generation > prev || …)` — writer and asserter are the same test-local statement.
**Fix:** Perform real document mutations through the editor's mutation API and assert `generation` advanced after each.

### `tests/wgpu_surface_integration.rs:32` — `document_state_can_be_shared_across_threads` — Category A — MEDIUM
**Defect:** Asserts a value the test itself just set (`generation = 1` via its own thread) — the only production property exercised is `DocumentState: Send`, which is a compile-time fact.
**Evidence:** the spawned closure does `s.generation = s.generation.wrapping_add(1);` and the test asserts `== 1`.
**Fix:** Either delete (the `Send` bound is checked by compilation) or assert a real cross-thread contract of `DocumentState` (e.g. layout swap visibility).

### `tests/wgpu_surface_integration.rs:1` — (whole file) — Category B — HIGH
**Defect:** File is named `wgpu_surface_integration` and lives in `tests/`, but contains zero wgpu/surface content — it smoke-tests `DocumentState` defaults, so a reader believes the GPU surface path has integration coverage when it has none.
**Evidence:** Only import is `loki_text::editing::state::DocumentState`; the doc comment itself says "Integration tests for the `DocumentState` API."
**Fix:** Rename the file to `document_state.rs` (or add actual surface-creation tests), so the absent wgpu coverage is visible as absent.

### `src/texture_budget_tests.rs:14` — (all 6 tests via `inputs_for`) — Category B — HIGH
**Defect:** Every test feeds `TextureBudget::derive` through a test-local `inputs_for` that *re-implements* the exact mapping `current()` performs (the `GpuClass::Unknown => None` judgment call included), so the production mapping in `texture_budget.rs:120-123` can regress without any test failing.
**Evidence:** test: `gpu_paint_path: match profile.gpu_class { GpuClass::Unknown => None, other => Some(other.allocates_page_textures()) }` — byte-for-byte the same match as production `current()`; the tests never call `current()`.
**Fix:** Extract the mapping into a pure `fn inputs_from_profile(&DeviceProfile, …) -> BudgetInputs` in production, call it from both `current()` and the tests, and assert on its output.

### `src/window_state.rs:38` — `initial_geometry_is_sane_without_a_persisted_file` — Category B — HIGH
**Defect:** Despite its name, the test doesn't establish the no-persisted-file condition — it reads whatever `window.json` exists on the host, and its assertion (`width >= 320`) holds for both the loaded and the fallback branch, so the fallback default is never actually pinned.
**Evidence:** `// Whatever is (or is not) on disk, the result is a usable size. let g = initial_geometry(); assert!(g.width >= 320.0 && g.height >= 240.0);`
**Fix:** Call `WindowGeometry::load` with a path known not to exist and assert the result equals `DEFAULT_GEOMETRY` (1280×800) exactly.

### `src/editing/navigation_tests.rs:143` — `navigate_home_returns_position_on_same_paragraph` — Category B — HIGH
**Defect:** On a single-line paragraph, Home from byte 6 must land at byte 0, but the assertion `byte_offset <= 6` is satisfied by an identity stub that returns the focus unchanged.
**Evidence:** `assert!(pos.byte_offset <= 6, "Home should move to start of line (byte ≤ 6)");` — 6 itself passes.
**Fix:** Assert `pos.byte_offset == 0` for the single-line fixture (and add a wrapped-line case asserting the line start, not the paragraph start).

### `src/editing/navigation_tests.rs:157` — `navigate_end_returns_position_on_same_paragraph` — Category B — HIGH
**Defect:** End from byte 0 of single-line `"hello world"` must land at byte 11, but the assertion `byte_offset > 0` passes for any forward movement, including a stub that returns the next grapheme.
**Evidence:** `assert!(pos.byte_offset > 0, "End should move past the start");`
**Fix:** Assert `pos.byte_offset == 11` (`"hello world".len()`).

### `src/editing/navigation.rs:149` — `navigate_up` / `navigate_down` positive paths — Category C — HIGH
**Defect:** The entire positive body of `navigate_up`/`navigate_down` (~80 lines: within-paragraph line hop, the Bug-3 `target_y >= margins.top` guard, rotation-aware mapping, and the cross-paragraph/cross-page fallback at lines 177-185 / 222-230) has zero tests — only the two `returns_none` guard tests exist.
**Evidence:** `navigate_up_at_first_line_returns_none` and `navigate_down_at_last_line_returns_none` are the only tests naming up/down; both use a single-line paragraph so the hit-test branch and fallback never execute. (The reflow twins *are* positively tested in `reflow_nav.rs`, which shows the gap is practical to close.)
**Fix:** A multi-line paragraph fixture asserting up/down moves exactly one line preserving x, plus a two-paragraph fixture asserting the cross-paragraph fallback lands on the adjacent paragraph's nearest line.

### `src/editing/touch.rs:202` — `word_at_boundary_between_words` — Category B — HIGH
**Defect:** The test's comment states the word must be preferred over the space at a boundary, but the assertion accepts either outcome, so deleting the word-preference logic (`is_word || result.is_none()` at touch.rs:125-131) cannot fail it.
**Evidence:** `assert!(result == Some((0, 5)) || result == Some((5, 6)), "offset at boundary must return either the word or the space…")` — the disjunction covers both branches of the code under test.
**Fix:** `assert_eq!(word_boundaries_at("hello world", 5), Some((0, 5)))` — the word wins, per the documented rule.

### `src/editing/touch.rs:146` — `tap_no_movement_short_duration` — Category B — MEDIUM
**Defect:** Named as a tap-classification test, but no code path ever produces `TouchPhase::Tap` (grep: the variant is matched in `touch.rs` and `editor_pointer_touch.rs` but constructed nowhere), so the test can only assert `Indeterminate` and the "Tap" semantics it names are untestable dead state.
**Evidence:** `assert_eq!(state.phase, TouchPhase::Indeterminate);` twice; no assignment `phase = TouchPhase::Tap` exists in the crate.
**Fix:** Either remove the dead `Tap` variant (rule 6: parked-vs-forgotten) or implement/assert the transition the test name promises.

### `src/editing/touch.rs:221` — `scroll_delta_100px_produces_100px_offset_change` — Category A — MEDIUM
**Defect:** The "scroll offset" arithmetic is computed inside the test (`delta = updated_y - last_y; new_offset = initial_scroll + delta`), and the assertion sits under two nested `if let`s that would silently skip it if the phase pattern stopped matching.
**Evidence:** the only production behavior pinned is that `update_move` stores `last_y = new_pos.1`; the 100-px "offset change" never touches the real scroll-apply code in `editor_pointer_touch.rs:154`.
**Fix:** Assert directly `state.phase == TouchPhase::Scroll { last_y: new_y }` after the move (unconditional), and test the actual delta application where it lives.

### `src/editing/caret_reveal.rs:124` — `caret_rect_paginated` / `caret_rect_reflow` — Category C — HIGH
**Defect:** The two functions that produce the caret's content-space rect — including the asymmetric margin addition the module doc flags as the S0.3 step-5 trap (`rect + para.origin + page.margins`) — have no tests; only the `PageStack` arithmetic and `should_reveal` are covered.
**Evidence:** grep shows the only non-test caller is `editor_caret_follow_geom.rs`; no test invokes either function, while the sibling `hit_test_tests.rs` proves the fixture machinery exists.
**Fix:** Reuse the hit-test layout fixture: assert `caret_rect_paginated` at byte 0 equals margins-offset content origin, and round-trip it through `hit_test_document` (the module doc claims they "must stay in agreement" — currently nothing enforces it).

### `src/editing/cursor.rs:124` — `CursorState::eq` — Category C — MEDIUM
**Defect:** The hand-written `PartialEq` deliberately includes `document_generation` (the comment says omitting it would stop Blitz repainting after formatting), but no test pins that; removing the field from `eq` fails nothing.
**Evidence:** `self.anchor == other.anchor && self.focus == other.focus && self.document_generation == other.document_generation` — cursor_tests.rs never compares two `CursorState`s.
**Fix:** Assert two states equal in anchor/focus but differing in `document_generation` compare unequal.

### `src/texture_budget.rs:47` — `override_bytes` / `ceiling_override_bytes` parsing — Category C — MEDIUM
**Defect:** The env-override parse rules (zero → ignored, garbage → ignored, MiB→bytes scaling) are documented as safety behavior ("a typo… should not blank the document") but untested; only the already-parsed `user_override_bytes` plumb-through is covered.
**Evidence:** `(mib > 0).then(|| mib * 1024 * 1024)` — no test sets `LOKI_TEXTURE_BUDGET_MB`; the `OnceLock` cache admittedly makes in-process testing awkward.
**Fix:** Extract `fn parse_override(raw: &str) -> Option<u64>` and assert `"0"`, `" 512 "`, `"abc"` → `None/Some(512 MiB)/None`.

### `src/editing/reflow_nav.rs:124` — `reflow_navigate_home` / `reflow_navigate_end` — Category C — MEDIUM
**Defect:** The reflow Home/End functions have no tests at all (the inline module covers left/right/up/down only), and `reflow_navigate_down`'s at-document-end → `None` contract is also unexercised.
**Evidence:** tests are `left_within_then_crosses_paragraph`, `right_crosses_into_next_paragraph`, `down_then_up_move_between_paragraphs` — nothing calls home/end.
**Fix:** Mirror the paginated tests: Home from mid-line asserts byte 0; End from byte 0 asserts `text.len()`; Down from the last paragraph asserts `None`.

### `src/editing/selection_handles.rs:82` — `grab_fixed_endpoint` focus branch + cell-path guard — Category C — MEDIUM
**Defect:** Only the anchor-handle grab is tested; the symmetric `near(focus) → Some(anchor)` branch and the `!pos.path.is_empty() → None` (no handles in table cells) guard are never exercised, so either could be deleted untested.
**Evidence:** `grabbing_a_handle_returns_the_opposite_endpoint` grabs only `anchor_grab`; no test builds a position with a non-empty `path`.
**Fix:** Grab at the focus handle's point and assert the anchor is returned; assert `handle_grab_point` returns `None` for a cell-path position.

### `src/utils.rs:90` — `percent_decode` edge cases — Category C — MEDIUM
**Defect:** The decoder's documented edge behavior — invalid/incomplete `%` sequences passed through, non-ASCII bytes rejected — has no test; only one happy-path `%20` case exists, and a valid-token-with-empty-display-name falling through to the path fallback (utils.rs:40) is also never exercised.
**Evidence:** tests cover `meeting%20notes.odt` only; `"file%zz.docx"`, trailing `"file%2"`, and `%C3` (non-ASCII) hit untested branches.
**Fix:** Assert `display_title_from_path("bad%zzname.docx")` and a trailing-`%2` name pass the sequence through unchanged.

Partition stats: 20 files read (plus 9 production counterparts traced), 126 tests examined, A=3 B=8 C=7 (18 findings).

Healthy areas: `page_locate_tests.rs` + the characterisation suite are exemplary — real flow-engine geometry, premise assertions, and a discriminator test that refutes a written-down inference (R9-18); `saved_state_tests.rs` locks the loro clean-point semantics against a real `UndoManager` including the unreachable-save-point case; `hit_test_tests.rs` inverts its own transform (missing-scroll-offset and zoom regression tests); `word_count_tests.rs`, `home_templates_tests.rs` (subset/inverse polarity pair), `startup_open_tests.rs`, `device_probe_tests.rs`, and `selected_object_tests.rs` all assert exact values on the production path with both polarities covered.
