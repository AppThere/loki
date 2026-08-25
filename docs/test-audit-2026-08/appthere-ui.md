# Test-efficacy audit — `appthere-ui`

Audited 2026-08-24. Scope: every `#[test]` in the crate — 53 test files
(sibling `*_tests.rs` + inline `#[cfg(test)]` mods; the crate has no `tests/`
integration directory), 378 tests. Method: read every test file in full; for
each suspicious test, read the production code it calls and applied the
mutation thought-experiment (stub the function / invert the guard — does the
test fail?). No repo files modified; no cargo runs.

Categories: **A** cannot fail · **B** doesn't test what it claims · **C** not
exhaustive (material gaps only). Confidence: HIGH = verified against production
source; MEDIUM = plausible, not fully traced.

---

## Findings

### A-1 — `detect_matches_the_compile_target` is a tautology
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/components/platform.rs:72-78`
- **Test:** `tests::detect_matches_the_compile_target`
- **Category:** A · **Confidence:** HIGH
- **Defect:** The test computes its expected value by executing the exact same
  expression the production function is defined as, so both sides are wrong
  together and the assertion can never discriminate.
- **Evidence:** Production (`platform.rs:29-31`):
  `pub fn detect() -> Self { Self::from_os_name(std::env::consts::OS) }`.
  Test: `assert_eq!(Platform::detect(), Platform::from_os_name(std::env::consts::OS));`
  If `from_os_name` misclassified every OS, both sides would still be equal.
  The only mutation it catches is `detect()` ceasing to delegate — a
  delegation pin at best, and the doc claims more ("detect matches the
  compile target").
- **Fixed test would assert:** under `#[cfg(target_os = "linux")]` (etc.),
  `Platform::detect() == Platform::Linux` — an expectation stated
  independently of the production expression. (The neighbouring
  `os_names_map_to_variants` already covers the mapping table itself.)

### A-2 — macOS budget-floor test exercises no production code
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/device_probe_tests.rs:119-125`
- **Test:** `the_macos_total_path_agrees_with_the_baseline_at_the_design_floor`
- **Category:** A · **Confidence:** HIGH
- **Defect:** The test is pure literal arithmetic — it restates the divisor
  (128) and the baseline (64 MiB) as its own literals and calls no production
  function, so no change to the real `TOTAL_RAM_DIVISOR` path can ever fail it.
- **Evidence:**
  ```rust
  let eight_gib = 8_u64 * 1024 * 1024 * 1024;
  assert_eq!(eight_gib / 128, 64 * 1024 * 1024);
  ```
  `TOTAL_RAM_DIVISOR` appears only in the doc comment (grep over `src/`
  confirms no code reference from this file); the budget derivation the test
  claims to pin lives outside the assertions entirely. Its own doc says the
  claim "should be a computation rather than a shrug" — but it is the test's
  computation, not the product's.
- **Fixed test would assert:** import the actual divisor/baseline constants
  (or call the budget function) and assert
  `budget_for(total = 8 GiB, available = None) == BASELINE_BUDGET`, so a
  changed divisor or baseline fails it.

### B-1 — `RootLayer` mount-order tests assert a table no production code reads
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/components/popover/host_tests.rs:12-29`
- **Tests:** `the_backdrop_mounts_below_the_overlay`, `each_layer_has_its_own_position`
- **Category:** B · **Confidence:** HIGH
- **Defect:** The test names claim a DOM/paint-order property ("the backdrop
  *mounts* below the overlay"), but the body only exercises
  `RootLayer::mount_order()` — an enum table that **no production call site
  consults**, so the real mounting order can invert while both tests stay
  green.
- **Evidence:** `grep -rn RootLayer` across the workspace: the only non-test
  references are a re-export (`popover/mod.rs:114`) and *comments* in
  `loki-text/src/app.rs:273`, `loki-presentation/src/app.rs:201`,
  `loki-spreadsheet/src/app.rs:201`. The actual ordering is enforced solely by
  rsx order inside `component_host.rs` (backdrop `div` emitted before the
  overlay `div`, lines 58-129). Swapping those two divs — the exact failure
  the test's message describes ("visible and unclickable... dead menu") —
  fails nothing.
- **Fixed test would assert:** the rendered order — either extend the existing
  `host_purity` source-assertion pattern in `component_host.rs` to check the
  backdrop block precedes the overlay block in `AtPopoverHost`'s body, or make
  the component actually consume `mount_order()` (rule 5: make the wrong
  thing unavailable) so the enum stops being a mirror.

### B-2 — `the_host_render_never_calls_place` disarms silently on rename
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/components/popover/component_host.rs:154-177`
- **Test:** `host_purity::the_host_render_never_calls_place`
- **Category:** B (guard can go vacuous; fixture precondition never asserted) · **Confidence:** HIGH (mechanism statically traceable)
- **Defect:** The source-scrape slices the file at the marker
  `"pub fn AtPopoverHost"` with `.nth(1).unwrap_or_default()`; if the function
  is renamed or the marker string drifts, the sliced body is `""` and
  `!"".contains("place(")` passes vacuously — the test keeps passing while
  guarding nothing.
- **Evidence:**
  ```rust
  let after = HOST.split("pub fn AtPopoverHost").nth(1).unwrap_or_default();
  let body = after.split("\n}\n").next().unwrap_or_default();
  ```
  The test's own doc history says a marker miss should fail loudly, and the
  crate's stated discipline (geometry_tests: "verify the fixture produces the
  precondition, in the test") is exactly what is missing here. The `\n}\n`
  slice is similarly fragile (first non-indented close brace wins).
- **Fixed test would assert:** `assert!(HOST.contains("pub fn AtPopoverHost"))`
  (and that the sliced body is non-empty and contains a known token from the
  render, e.g. `"onkeydown"`) before asserting the absence of `place(` — a
  positive control that the slice found its subject.

### C-1 — `status_bar_overflow.rs`: pure size/placement logic with zero tests
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/components/status_bar_overflow.rs:92-118`
- **Category:** C · **Confidence:** HIGH
- **Defect:** `menu_size` and `menu_placement` are pure, testable functions
  (the crate's own convention is to extract and test exactly this kind of
  decision — the *ribbon* overflow menu next door has `overflow_menu_tests.rs`
  for its analogous pair) but have no tests at all; deleting the
  `MIN_MENU_WIDTH_PX` floor, the padding terms, or flipping `Side::Above` /
  `Align::End` fails nothing.
- **Evidence:** `menu_size` folds `estimate_label_px` over rows with a
  `max(MIN_MENU_WIDTH_PX)` floor; `menu_placement` hard-codes
  `Side::Above`/`Align::End`. No `#[cfg(test)]` in the file and no sibling
  `status_bar_overflow_tests.rs` exists. Also untested: the custom
  `PartialEq` for `StatusOverflowRow` (lines 75-84), whose
  handler-presence-only comparison is a render-loop-prevention decision.
- **Fixed test would assert:** mirror `overflow_menu_tests.rs` — width follows
  the widest label and floors at `MIN_MENU_WIDTH_PX`; height follows the row
  count; empty rows are non-degenerate; placement is Above/End with the
  requested size reaching the placement.

### C-2 — `AtDialogButton`: disabled/precedence branches untested
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/components/dialog/button.rs:42-101`
- **Category:** C · **Confidence:** HIGH
- **Defect:** The file's only test pins `PRIMARY_TOUCH_PX > TOUCH_MIN`; the
  component's actual decisions — disabled wins over primary in the style
  chain, the click handler swallows activation when `disabled`, and the
  primary-height substitution applies only when `min_touch_px > 0.0 && primary`
  — have no coverage, and none of it needs a runtime (the style/handler logic
  could be extracted pure per the crate's own pattern).
- **Evidence:** the `if disabled {...} else if primary {...} else if tertiary`
  chain and `onclick: ... if !disabled { on_click.call(()) }` are only
  exercised by rendering. Deleting the `!disabled` guard or reordering the
  chain (primary before disabled) fails no test.
- **Fixed test would assert:** extract the `(bg, border, fg)` decision and the
  touch-height decision into pure fns and assert: disabled+primary yields the
  disabled palette; `min_touch_px = 0` never emits a min-height even for
  primary; a disabled activation is not forwarded.

### C-3 — Macro-security prompt components have no extracted, tested decisions
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/components/macro_security/` (537 lines: `trust.rs`, `permission.rs`, `network.rs`, `frame.rs`, `mod.rs`)
- **Category:** C · **Confidence:** MEDIUM
- **Defect:** Security-adjacent UI (trust dialog, permission prompt, network
  prompt) carries zero tests and — unlike the rest of the crate — has no
  extracted pure decision layer, so properties like "which action is the
  accented/default one" or "what dismissal maps to" are unpinned; a swap of
  the safe and dangerous defaults would fail nothing.
- **Evidence:** the only non-component fn is `mod.rs:62 choice_button_style(accented: bool)`;
  all four components are rsx-only with no sibling `*_tests.rs`. (MEDIUM
  because the underlying trust *decisions* may live in the consuming app; the
  gap here is the default/accent/dismissal mapping this crate does own.)
- **Fixed test would assert:** per prompt, an extracted table of
  (action → accented?, action → is_default?, dismiss → which outcome), with
  the polarity that the dangerous action is never the accented default.

### C-4 — `GpuClass::Unknown.allocates_page_textures()` — parked branch, comment-only marking
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/device_profile_gpu.rs:77-86`
- **Category:** C · **Confidence:** HIGH
- **Defect:** The `Unknown => false` arm is documented as "the caller must not
  reach here" but no test pins either the value or the unreachability
  contract; the marking is a comment only, which the project's own evidence
  rules (rule 6: marking must be mechanical) call out as the decayed form.
- **Evidence:** `device_profile_tests.rs` covers `Software` and `None` for
  `allocates_page_textures` but never `Unknown`; changing the arm to `true`
  (silently joining the paint side) fails nothing.
- **Fixed test would assert:** `!GpuClass::Unknown.allocates_page_textures()`
  with a message naming the L9-009 contract ("not probed" is not "no GPU" —
  callers must map Unknown before asking).

### C-5 — `key_from_parts`: Shift+letter typeahead never asserted
- **Crate/file:** `appthere-ui` — `/home/kdesltd/project/loki/appthere-ui/src/components/popover/key_event_tests.rs` (vs `key_event.rs:38`)
- **Category:** C · **Confidence:** HIGH
- **Defect:** `a_modified_key_is_not_ours` sweeps CONTROL/ALT/META but never
  SHIFT on a character, so the deliberate carve-out (`if mods.ctrl() ||
  mods.alt() || mods.meta()` — Shift excluded) that makes uppercase typeahead
  work is untested in the typeahead direction.
- **Evidence:** a mutation adding `|| mods.shift()` to the guard would be
  caught only via Shift+Tab (`shift_tab_is_distinguished_from_tab`), not via
  the typeahead path; a narrower regression (Shift+Character → None) is
  invisible to the suite.
- **Fixed test would assert:**
  `press_with(Character("D"), SHIFT) == Some(Key::Char('D'))`.

### C-6 — Runtime-bound plumbing with no coverage (inventory, low urgency)
- **Crate/file:** `appthere-ui` — `src/safe_area.rs` (90 ln), `src/device_profile_apply.rs` (45 ln), and the rsx-only shell components (`status_bar.rs`, `tab_bar.rs`, `title_bar.rs`, `confirm_dialog.rs`, `template_browser.rs`, `infobar.rs`, `overlay.rs`, `document_tab.rs`, `icons.rs`)
- **Category:** C · **Confidence:** MEDIUM
- **Defect/scope:** All untested, but consistent with the crate's declared
  pattern (decisions extracted and tested; components need a Dioxus runtime).
  The pieces with any real decision content: `safe_area`'s seed-vs-update
  semantics (seed does not notify; update does) and
  `apply_profile_override`'s forced-fields-stand merge. Listed for inventory,
  not as urgent gaps; the extracted logic these components consult *is*
  tested elsewhere.

---

## Statistics

| Metric | Value |
|---|---|
| Test files read | 53 of 53 |
| Tests examined | 378 |
| Category A findings | 2 (both HIGH) |
| Category B findings | 2 (both HIGH) |
| Category C findings | 6 (4 HIGH, 2 MEDIUM) |
| Production files read for verification | 12 (platform, host, component_host, key_event, device_profile_gpu/apply, safe_area, status_bar_overflow, dialog/button+field+provenance, convert, loki-text app.rs) |

## Healthy areas

This is the strongest test suite of any crate I have audited. The failure
modes this audit hunts are not just absent — they are *named and countered*
in the tests themselves:

- **Polarity discipline is systematic.** Dozens of tests carry an explicit
  "L08-045" inversion partner so a constant-returning stub cannot pass (e.g.
  `custom_source_tests::the_fields_are_not_simply_ignored`,
  `wiring_tests`' three-way dismiss-on-unmount polarity,
  `presentation_tests::an_ordinary_anchor_stays_anchored`).
- **Fixture preconditions are asserted in-test** (geometry/interaction
  suites: every "near the bottom" fixture first proves it still creates the
  condition), which is precisely the discipline B-2 above fails to apply to
  itself.
- **Negative controls on predicates**:
  `geometry_tests::the_containment_predicate_can_say_no_on_every_edge` and
  `the_covering_predicate_fires_...` exist specifically because a sweep of
  hundreds of thousands of assertions was once vacuous through one
  too-permissive predicate — and the tests document it.
- **Mutation-derived cases** are recorded as such
  (`a_preference_that_fits_is_honoured_over_a_roomier_alternative` was found
  by mutation testing; `interaction_tests::a_menu_handles_every_key_itself`
  explicitly replaced an assert-free predecessor).
- Pure-logic extraction (`*_tests.rs` beside every decision module) keeps the
  untestable rsx layer thin; the zoom, scroll, responsive, popover, dialog,
  color-picker, and device-probe suites are exemplary and I found no A/B
  defect in any of them.
