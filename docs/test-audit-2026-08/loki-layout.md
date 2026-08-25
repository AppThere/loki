# Test-efficacy audit — loki-layout

Audited 2026-08-25. Scope: every `#[test]` in `loki-layout` (46 test files, ~389 tests):
`#[cfg(test)]` mods, sibling `*_tests.rs` files, and `tests/` integration tests. Method:
full read of every test file, then verification of each suspicious test against the
production code it calls (mutation thought-experiment: would the test fail if the
production guard were inverted / the function stubbed?). No repo files modified; no
builds run.

Categories: **A** = cannot fail regardless of production behavior; **B** = doesn't test
what it claims; **C** = material coverage gap.

---

## Findings

### F1 — A — tautological assertion `!warnings.is_empty() || warnings.is_empty()`
- **Crate/file:** loki-layout, `src/flow_tests.rs:1692-1695`, test `list_items_produce_glyph_runs`
- **Confidence:** HIGH
- **Defect:** One of the test's assertions is a literal tautology that passes for every possible value.
- **Evidence:**
  ```rust
  assert!(
      !warnings.is_empty() || warnings.is_empty(),
      "warnings either way is fine"
  );
  ```
- **Fix:** Delete the assertion, or decide what the warnings contract for a list section actually is (e.g. `assert!(warnings.is_empty())`) and assert that.

### F2 — A — `keep_together_block_pushed_to_next_page` passes with the keep-together branch deleted
- **Crate/file:** loki-layout, `src/flow_tests.rs:1126-1154`, test `keep_together_block_pushed_to_next_page`
- **Confidence:** HIGH (verified against `src/flow_para_place.rs:87-119` and the crate's own Carlito metric of 14.648 pt/line documented at `src/flow_tests.rs:420-424`)
- **Defect:** The fixture makes para2 overflow the page *naturally*, so the test cannot distinguish keep-together from ordinary line overflow — and para2 is single-line, which never splits anyway.
- **Evidence:** para1 (one 14.648 pt line) + `space_after: 78.0` = 92.6 pt cursor on a 90 pt content page, so para2 (needing ~14.6 pt) flows to page 2 through the ordinary overflow path whether or not `resolved.keep_together` is consulted. The keep-together branch (`flow_para_place.rs:87`) only changes behavior for a **multi-line** paragraph that *partially* fits; this fixture exercises neither condition. Assertions are `pages.len() == 2` and `!pages[1].content_items.is_empty()` — both hold under the mutation "delete the `if resolved.keep_together` branch".
- **Fix:** Fixture where the keep block is multi-line and its first lines *do* fit in the remaining space (e.g. 3 lines with 2 lines of room): keep-together must move all lines to page 2 (assert page 1 carries no fragment of it via `editing_data`/`glyph_x_origins`), and the same fixture with `keep_together` unset must split.

### F3 — A — `keep_with_next_chain_pushed_to_next_page` passes with the chain logic deleted
- **Crate/file:** loki-layout, `src/flow_tests.rs:1388-1418`, test `keep_with_next_chain_pushed_to_next_page`
- **Confidence:** HIGH (same fixture arithmetic as F2; chain code in `src/flow_run.rs:222-226` / `src/flow_para_chain.rs`)
- **Defect:** The comment claims "para1 and para2 must be on the same page (page 2)" but no assertion checks it; and the fixture puts para1 on page 2 by natural overflow (para0 line + 78 pt after = 92.6 > 90), so the asserted properties hold even if `flow_keep_with_next_chain` never runs.
- **Evidence:**
  ```rust
  // para1 and para2 must be on the same page (page 2).
  assert!(!pages[1].content_items.is_empty(), "page 2 must have content");
  ```
  Remaining room after para0 is 90 − 14.6 − 78 < 0, so para1 cannot fit on page 1 regardless of keep-with-next.
- **Fix:** Fixture where para1 *fits* at the page bottom on its own (e.g. `space_after ≈ 55` leaving ~1.5 lines of room): with keep_with_next the heading must move to page 2 with its body (assert the heading's glyphs/editing entry are on page 2); without the flag it must stay on page 1.

### F4 — B — `split_fragment_b_clip_starts_at_top_of_next_page` asserts `y >= 0`, not "top"
- **Crate/file:** loki-layout, `src/flow_tests.rs:1002-1027`, test `split_fragment_b_clip_starts_at_top_of_next_page`
- **Confidence:** HIGH
- **Defect:** The name promises the continuation fragment's clip starts at the top of page 2, but the assertion accepts any non-negative y — a regression placing fragment B at y = 40 passes.
- **Evidence:**
  ```rust
  assert!(clip_b.y() >= 0.0, "Fragment B clip_rect.y must be ≥ 0 (page-local)");
  assert!(clip_b.height() > 0.0, "Fragment B clip height must be positive");
  ```
- **Fix:** `assert!(clip_b.y().abs() < 0.5)` (fragment B resumes at the content top), keeping the positive-height check.

### F5 — B — list-counter tests cannot detect a counter that never advances
- **Crate/file:** loki-layout, `src/flow_tests.rs:1706-1730` `numbered_list_counters_advance`; `src/flow_tests.rs:1751-1777` `new_list_resets_counter`
- **Confidence:** HIGH (verified production: `FlowState::advance_counter` at `src/flow.rs:165-180`; new-list reset `state.list_counters.remove(&lm.list_id)` at `src/flow_list_marker.rs:137-139`)
- **Defect:** Both tests assert only a minimum glyph-run *count* (`runs >= 3` / `runs >= 4`); stubbing `advance_counter` to always return `start_value` (every marker "1."), or deleting the new-list reset, changes no run count — both tests still pass.
- **Evidence:**
  ```rust
  // numbered_list_counters_advance:
  assert!(runs >= 3, "three list items must produce ≥3 glyph runs, got {runs}");
  ```
  The tests' own comments concede it: "we can't inspect the text directly from PositionedItems … the test verifies that the flow engine does not panic".
- **Fix:** Discriminate via glyph counts (the technique `tests/header_variants.rs` already uses): 10 items where item 10's marker "10." shapes to 3 glyphs vs "1."'s 2; or run with `preserve_for_editing` and read the flattened marker text.

### F6 — B — `counter_advance_single_list` / `counter_nested_deeper_reset` test formatting, not counters
- **Crate/file:** loki-layout, `src/para_tests.rs:1594-1620`
- **Confidence:** HIGH
- **Defect:** Both tests hand-construct the counter array and call `format_list_marker` — the advancement (`counters[lvl] += 1`), start-value seeding, and deeper-level reset (`flow.rs:176-178`) they are named for are never executed; they are duplicate coverage of `format_list_marker` (already covered by the `format_marker_*` tests directly above them).
- **Evidence:**
  ```rust
  // "When level 0 advances, level 1 should have been reset to 0.
  //  We simulate: level 0 = 2, level 1 = 0 (reset) then first use = 1."
  let c = counters(&[(0, 2), (1, 1)]);
  assert_eq!(format_list_marker(&levels, 1, &c), "2.1.");
  ```
  The test *asserts on a value it constructed itself* — the production reset could be deleted and this passes.
- **Fix:** Unit-test `FlowState::advance_counter` directly (it is `pub(super)`): advance level 0 twice, then level 1, and assert the returned values and that `counters[1]` was zeroed by the level-0 advance.

### F7 — C — the entire list-counter machinery has no discriminating test
- **Crate/file:** loki-layout, `src/flow.rs:165-180` (`advance_counter`), `src/flow_list_marker.rs:131-146` (start-value selection, new-list reset)
- **Confidence:** HIGH (combined effect of F5 + F6; grep confirms no other caller-side coverage)
- **Defect:** Increment vs. re-seed (`counters[lvl] == 0` guard), `start_value` seeding, deeper-level zeroing, and the new-list reset are all unobserved by any test — four named tests orbit this code and none would fail if any of those behaviors were broken.
- **Fix:** As F5/F6. This is the single highest-value gap in the crate: numbering restarts and nested numbering are user-visible in every DOCX list.

### F8 — B — `superscript_is_raised_and_narrower` (math) detects neither raise nor shrink
- **Crate/file:** loki-layout, `src/math/math_tests.rs:74-84`, test `superscript_is_raised_and_narrower`
- **Confidence:** HIGH
- **Defect:** The name claims the superscript is raised and narrower; the assertions are `count == 2`, `sup.ascent >= base.ascent` (non-strict — equal passes), and `sup.width > base.width` (the *box* is wider, which holds trivially by adding a glyph). A typesetter that renders the exponent at full size on the baseline passes.
- **Evidence:**
  ```rust
  assert_eq!(count_glyph_runs(&sup.items), 2);
  assert!(sup.ascent >= base.ascent);
  assert!(sup.width > base.width);
  ```
- **Fix:** Assert the superscript glyph run's `font_size` < the base run's, and its `origin.y` < the base run's baseline y (raised). Compare to `src/para_tests.rs::super_and_subscript_use_words_measured_size_and_shift`, which does this properly for text runs.

### F9 — B — `last_page_starting_mid_paragraph_keeps_fill_first` never checks columns
- **Crate/file:** loki-layout, `src/flow_tests.rs:589-611`
- **Confidence:** HIGH
- **Defect:** Name and doc promise the mid-paragraph tail keeps fill-first column packing; the body asserts only `pages.len() >= 2` and every page non-empty — nothing about column x-bands, so balanced (or single-column) packing of the tail also passes.
- **Evidence:**
  ```rust
  assert!(pages.iter().all(|p| !p.content_items.is_empty()), "every page keeps its content");
  ```
- **Fix:** Reuse `glyph_x_origins` on the last page and assert runs exist at x < 50 and (for a full page) x ≥ 99, as the sibling `multi_page_two_column_section_keeps_fill_first_on_every_page` does. If only no-panic/no-loss is intended, rename to say so.

### F10 — A — all six `measure_tests` silently pass on hosts without Liberation/DejaVu
- **Crate/file:** loki-layout, `src/measure_tests.rs:20-32` (`resources()`), affecting all 6 host-font tests in the file
- **Confidence:** MEDIUM (verified the early-return; whether it fires depends on the host)
- **Defect:** `resources()` returns `Some` only when a *host* font file is readable; every test begins `let Some(mut r) = resources() else { return };` — on macOS or a slim container all six tests pass while executing zero assertions, despite `with_bundled_fonts_only()` already registering Carlito which the measurement could use.
- **Evidence:**
  ```rust
  for p in [".../LiberationSans-Regular.ttf", ".../DejaVuSans.ttf"] {
      if let Ok(data) = std::fs::read(p) { r.register_font(data); return Some(r); }
  }
  None
  ```
- **Fix:** Measure against the bundled "Carlito" face and drop the `Option` — the file's own doc comment ("a null result dressed as a pass") describes exactly the hazard the skip re-introduces.

### F11 — B — `hit_test_midpoint_is_between_start_and_end` asserts only the upper bound
- **Crate/file:** loki-layout, `src/para_tests.rs:1747-1762`
- **Confidence:** HIGH
- **Defect:** Name says "between start and end"; the only assertion is `hit.byte_offset < text.len()` — a hit-test regression mapping every click to offset 0 passes (and also passes `hit_test_at_origin_returns_offset_zero`; only the far-right clamp test would object, for clicks nowhere near the middle).
- **Evidence:**
  ```rust
  let hit = result.hit_test_point(mid_x, mid_y).expect("must return Some");
  assert!(hit.byte_offset < text.len(), ...);
  ```
- **Fix:** `assert!(hit.byte_offset > 0 && hit.byte_offset < text.len())`, or better, assert a tolerance band around the expected mid-string offset.

### F12 — B — `bold_span_produces_items` cannot detect bold being dropped
- **Crate/file:** loki-layout, `src/para_tests.rs:183-220`
- **Confidence:** MEDIUM (bold synthesis is covered indirectly: `tests/variable_font_weight.rs` pins the wght coords; `src/resolve_tests.rs` pins span.bold propagation)
- **Defect:** The fixture builds a 3-span paragraph with a bold middle run, then asserts only `!items.is_empty()` and `runs >= 1` — nothing bold-specific; the same assertions pass for plain spans.
- **Evidence:**
  ```rust
  assert!(!result.items.is_empty());
  assert!(runs >= 1, "expected at least one glyph run, got {runs}");
  ```
- **Fix:** Assert the middle run's `synthesis`/`normalized_coords` differ from the flanking runs (or that the bold range shapes to a distinct run at weight 700). Alternatively rename to a smoke test; the discriminating coverage lives in `variable_font_weight.rs`.

### F13 — B — `space_before_offsets_content` bound admits partial application
- **Crate/file:** loki-layout, `src/flow_tests.rs:648-672`
- **Confidence:** MEDIUM
- **Defect:** Asserts `first_glyph_y >= space_before` (24 pt). Baseline y = applied_space + ascent (~11.7 pt), so the test fails only when less than ~52% of `space_before` is applied — e.g. a bug applying 15 of 24 pt passes.
- **Evidence:**
  ```rust
  assert!(y >= space_before, "first glyph run y ({y}) should be ≥ space_before ({space_before})");
  ```
- **Fix:** Comparative form (as the collapse tests in the same file do): lay out with `space_before` 0 and 24, assert the delta is 24 ± ε.

### F14 — C — `flow_line_numbers::emit` is untested; only the trivial counter-state struct is
- **Crate/file:** loki-layout, `src/flow_line_numbers.rs:65-172` (production `emit`/`paint_number`); tests at `:205-247` cover only `LineNumberState::new`/`restart_for_page`
- **Confidence:** HIGH (grep over the crate: no other test constructs `LineNumbering` or reaches `emit`)
- **Defect:** The feature's substance — per-line counter advance, `count_by` selection (`n.rem_euclid(count_by)`), midpoint fragment-membership bucketing, the empty-paragraph line, right-alignment of the number at content-local `x = -distance - width`, the single-column/paginated/no-table gate, and the per-page `restart_for_page` *integration* — has zero coverage. Any of these could be inverted and no test fails.
- **Fix:** One paginated flow test with `LineNumbering { count_by: 2, restart: NewPage, .. }` over a two-page section: assert glyph runs at negative x exist only beside every second line, numbers restart on page 2, and a two-column variant emits none.

### F15 — C — no fixture makes keep-together/keep-with-next *placement* observable
- **Crate/file:** loki-layout, `src/flow_para_place.rs:87-119`, `src/flow_para_chain.rs`
- **Confidence:** HIGH (combined effect of F2 + F3)
- **Defect:** The "fits → 1 page" tests pass trivially (everything fits anyway) and the "pushed → 2 pages" tests pass by natural overflow (F2/F3), so the positive half of both features — deliberately flushing a page early to keep a block/chain intact — is exercised by no test. The chain's *warning* paths (`KeepWithNextChainTruncated`, `KeepWithNextChainTooTall`) and its image/footnote-preservation regressions are covered; the core placement decision is not.
- **Fix:** As F2/F3 — fixtures where the block/chain head fits in the remaining space and the keep flag must move it anyway, each paired with its flag-off inversion.

### F16 — B (minor) — stale `#[ignore]` doc on a live test
- **Crate/file:** loki-layout, `tests/table_tests.rs:450-456`, test `fixed_columns_should_be_honored_like_word`
- **Confidence:** HIGH
- **Defect:** The doc comment says the test is `#[ignore]`d "until `w:tblLayout` is parsed" and to "remove `#[ignore]` once the fixed-layout path exists", but no `#[ignore]` attribute is present and the test uses the now-existing `TABLE_FIXED_LAYOUT_CLASS`. The test itself is sound; the header misdescribes both its status and (partly) the sibling characterization test's rationale.
- **Fix:** Rewrite the doc comment to reflect that the fixed-layout path landed and the test is live.

---

## Per-crate stats

| Metric | Value |
|---|---|
| Test files read | 46 (31 in `src/`, 15 in `tests/`) |
| Tests examined | ~389 |
| Findings — A (cannot fail) | 4 (F1, F2, F3, F10) |
| Findings — B (doesn't test what it claims) | 9 (F4, F5, F6, F8, F9, F11, F12, F13, F16) |
| Findings — C (material gaps) | 3 (F7, F14, F15) |
| HIGH confidence | 13 · MEDIUM: 3 |

Note: F5+F6 feed the gap F7, and F2+F3 feed F15 — the distinct defective *areas* are:
list counters, keep-together/keep-with-next placement, fragment-B clip position, math
superscript metrics, measure-test host skip, line numbering, plus five weak single tests.

## Healthy areas

The crate's test quality is well above average overall; most files show deliberate
inversion discipline (guard-false controls, polarity tests, anti-vacuity assertions):

- **Geometry with real discrimination:** column flow (`unequal_columns…`, fill-first
  suite), spacing collapse (`adjacent_paragraph_spacing_collapses_to_the_larger` with
  larger-before inversion), forced- vs flow-break `space_before` (four-arm measured
  table), borders (`a_border_occupies_its_space_and_width` splits width/space terms so
  neither can be dropped), super/subscript ratios pinned to Word-measured constants
  with a sign-error inversion, drop-cap `lines`-is-not-an-input with a degenerate-band
  guard, tab stops (right/center/decimal with equal-suffix trick), hanging indents.
- **Anti-vacuity guards as tests:** `the_sweep_still_exercises_a_boundary`
  (spell-squiggle condition sweep), `assert_no_squiggle_is_cut` refusing empty inputs,
  `incremental_tests`' `fired` flags, and the explicitly self-limiting
  `the_property_tests_only_ever_resume_from_block_zero` — a mechanical tripwire that
  documents the incremental suite's resume-from-block-0 limitation and will fail when
  T3.4 lands (a model example of a *marked* deferral).
- **Cache tests with mutation-hardened assertions:** `para_cache_tests` (the
  eviction-vs-starvation inversion documents two weaker drafts a `clear()` mutant
  passed), Arc-identity sharing tests (`para_layout_sharing_tests`) including the
  copy-on-write hazard.
- **Font substitution / variable fonts / kerning:** table-driven, records-checked,
  with unknown-family controls and font-table-derived expected advances.
- **Header/footer variant selection** (`header_variants.rs`): glyph-count labeling
  with an explicit hardwired-`is_first` counter-fixture.
- **Boundary coverage** is generally good: empty paragraph, empty map, zero-size
  image, zero/negative column width, cyclic style chains, line-taller-than-page
  termination, row-taller-than-page split closure.
