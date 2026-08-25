# Test-efficacy audit — loki-doc-model

Scope: all `#[test]` tests in `loki-doc-model` (29 integration files in `tests/`, ~60 unit/sibling test files in `src/`; 522 tests counted: 255 integration + 267 unit).
Method: read every test file; for each suspicious test, read the production code it exercises; applied the mutation thought-experiment ("would this fail if the code under test were stubbed/inverted?").

---

## Findings

### F1. `test_bold_run` never checks the bold mark it is named for
- Crate: loki-doc-model — `tests/loro_bridge_tests.rs:36–50` — test `test_bold_run`
- Category: **B** — Confidence: **HIGH**
- Defect: the test's only assertion is that the flattened text appears in the serialized JSON; the bold mark is never verified, and the in-file comment admits it ("we just ensure it didn't panic").
- Evidence:
  ```rust
  let json = doc_to_json_string(&doc);
  assert!(json.contains("Normal bold"));
  // Since bold mark is an annotation it might appear in op logs or delta, we just ensure it didn't panic.
  ```
  A writer that silently dropped the `MARK_BOLD` annotation entirely would pass. (The later `test_roundtrip_bold_mark` covers the round trip, so this test adds nothing it claims to.)
- Fix: either delete it as redundant with `test_roundtrip_bold_mark`, or assert the mark is present on the Loro side (e.g. via the rich-text delta / `first_block_type`-style probe as in `loro_bridge_opaque_tests.rs`).

### F2. `test_coloured_run` asserts only the text, not the colour
- Crate: loki-doc-model — `tests/loro_bridge_tests.rs:84–107` — test `test_coloured_run`
- Category: **B** — Confidence: **HIGH**
- Defect: named "coloured run", but the sole assertion is `assert!(json.contains("color text"))` — the colour is never checked; a writer dropping `CharProps.color` passes.
- Evidence:
  ```rust
  let json = doc_to_json_string(&doc);
  assert!(json.contains("color text"));
  ```
- Fix: redundant with `roundtrip_rgb_color_mark` (same file) — delete, or assert the colour mark reaches the CRDT representation.

### F3. Gutter margin asserted at its default value — non-discriminating
- Crate: loki-doc-model — `tests/loro_bridge_tests.rs:206–235` — test `roundtrip_page_layout_non_default_margins`
- Category: **B** — Confidence: **HIGH**
- Defect: the fixture sets `gutter: Points::new(0.0)` — the default — then asserts `(gutter - 0.0).abs() < eps`. A bridge that never serializes gutter and reconstructs the default passes; the one margin field in this test that gets no discriminating value is the only assertion that cannot fail.
- Evidence:
  ```rust
  gutter: Points::new(0.0), ...
  assert!((layout.margins.gutter.value() - 0.0).abs() < eps);
  ```
- Fix: set gutter to a non-zero value (e.g. 21.6pt) and assert that. Every other margin in the test uses a non-default value; gutter should too.

### F4. `test_three_paragraphs` cannot detect paragraph merging
- Crate: loki-doc-model — `tests/loro_bridge_tests.rs:66–81` — test `test_three_paragraphs`
- Category: **B** — Confidence: **HIGH**
- Defect: asserts only `json.contains("P1"/"P2"/"P3")` on the serialized deep value; a writer that flattened all three paragraphs into one block would pass. The block *count* — the property the name implies — is never asserted.
- Evidence:
  ```rust
  assert!(json.contains("P1")); assert!(json.contains("P2")); assert!(json.contains("P3"));
  ```
- Fix: count `"type":"para"` occurrences or, better, assert `sections[0].blocks.len() == 3` after `loro_to_document` (as `test_roundtrip_hello_world_para` does for one block).

### F5. Header/footer bridge coverage: only the Default header slot is ever tested
- Crate: loki-doc-model — `src/loro_bridge/page_layout.rs:57–62` vs. `tests/loro_bridge_tests.rs:254` (`roundtrip_section_with_header_content`)
- Category: **C** — Confidence: **HIGH**
- Defect: the writer serializes six slots — `header`, `footer`, `header_first`, `footer_first`, `header_even`, `footer_even` — but the whole suite round-trips only `layout.header` with `HeaderFooterKind::Default`. A repo-wide grep for `footer`, `header_first`, `header_even` in test files finds zero hits. A reader that ignored (or cross-wired) the other five slots would pass every test; footers vanishing on the first CRDT re-derive (i.e., every keystroke) would be invisible.
- Evidence: `map_header_footer_slot(&layout.footer, KEY_FOOTER, ...)` etc. have no `tests_for`; only `KEY_HEADER` content is asserted anywhere.
- Fix: extend `roundtrip_section_with_header_content` into a six-slot round-trip test (distinct sentinel text per slot, plus `HeaderFooterKind` values `First`/`Even`), and assert each slot returns with its own text (cross-wiring detection).

### F6. `merged_with_parent` hand-maintained field lists are only spot-checked
- Crate: loki-doc-model — `src/style/props/char_props.rs:248–287`, `src/style/props/para_props.rs:218–258`; tests `src/style/props/char_props_tests.rs:17–34` (`merge_child_wins_for_some`), `src/style/props/para_props.rs` (`merge_inherits_parent_alignment`)
- Category: **C** — Confidence: **HIGH**
- Defect: both merge functions enumerate every field by hand via an `inherit!(field)` macro line (30 lines for `CharProps`, 26 for `ParaProps`). Nothing forces the list to stay complete when a field is added — no struct destructuring, no exhaustiveness check — and the tests exercise 3 of 30 (char) and 1 of 26 (para) fields. Deleting e.g. `inherit!(underline)` or `inherit!(border_top)` kills style inheritance for that property and no test dies.
- Evidence:
  ```rust
  inherit!(font_name); ... inherit!(hyperlink);   // 30 hand-written lines
  // test asserts only font_name, bold, font_size
  ```
- Fix: rewrite `merged_with_parent` over a destructuring `let Self { field1, field2, .. } = ...` (compile error on new fields), or add a test that constructs a fully-populated parent, merges with a default child, and asserts the result equals the parent for all Option fields (serde-JSON comparison makes this ~10 lines and future-proof).

### F7. Revision-mark wire format has no pinned golden value
- Crate: loki-doc-model — `src/style/props/revision_tests.rs` (all 4 tests) and production `encode`/`decode` in `src/style/props/revision.rs`
- Category: **B** — Confidence: **MEDIUM**
- Defect: all encode/decode tests are symmetric (`decode(&encode(&mark))`) within the same module. The encoding is persisted inside Loro CRDT state (snapshots on disk, server oplogs), so a change to the format passes every test while breaking every existing document's tracked changes. Only the `\u{1f}` separator is indirectly pinned by `an_unknown_tag_is_rejected`.
- Evidence:
  ```rust
  assert_eq!(decode(&encode(&mark)), Some(mark));  // both halves from the module under test
  ```
- Fix: one golden assertion, e.g. `assert_eq!(encode(&mark), "d\u{1f}Ada\u{1f}...")`, and a decode-from-literal test — the pattern `loro_bridge/decode_tests.rs::v1_border_strings_still_decode` already applies to borders.

### F8. Reader error/robustness paths are never exercised
- Crate: loki-doc-model — `loro_bridge::loro_to_document` / `BridgeError` (src/loro_bridge/mod.rs:63–75); whole suite
- Category: **C** — Confidence: **HIGH**
- Defect: every one of the ~90 call sites of `loro_to_document` in tests expects `Ok`; grep finds no `is_err`/`expect_err` on it anywhere. All reader input is produced by this crate's own writer in the same process. A malformed or version-skewed CRDT doc (hand-edited map, missing `KEY_TYPE`, wrong container type, truncated opaque JSON) — exactly what a remote peer or older client can produce in a collaboration product — hits code whose behaviour (graceful skip vs. error vs. panic) is untested.
- Evidence: `grep -rn "loro_to_document(...).expect_err" tests/ src/` → no matches.
- Fix: a small adversarial suite: seed a valid doc, then corrupt one thing at a time via raw Loro APIs (delete `KEY_TYPE`, replace a block map with a string, write invalid JSON into an opaque snapshot / style catalog) and assert the documented outcome (block skipped or typed error — never a panic, never dropping *other* blocks). The styles/settings readers silently `.ok()` parse failures — asserting that fallback is deliberate would also satisfy Evidence rule 6 (marking parked behaviour).

### F9. Tautological constructor tests (cluster) — assert what the test just built
- Crate: loki-doc-model — Category: **A** — Confidence: **HIGH**
- The following tests construct a struct/enum literal and assert the fields they just wrote (or a derived `Default`); they cannot fail against any production-code mutation short of changing the type definition, which the compiler already enforces:
  - `src/content/inline.rs` — `inline_str`, `inline_styled_run_with_props`, `inline_note_stores_blocks`, `inline_note_endnote_kind`
  - `src/content/block_tests.rs:8–30` — `heading_level_one`, `styled_para_with_style_ref`
  - `src/content/annotation/comment.rs` — `comment_author` (sets `c.author` then reads it back); `comment_ref_start_end`
  - `src/content/annotation/tracked.rs` — `tracked_change_with_author` (same pattern)
  - `src/content/table/core.rs` — `table_two_by_two` (counts rows/cells it constructed; only `col_count()` touches logic)
  - `src/content/table/row.rs` — `cell_simple_no_span`, `row_from_cells`
  - `src/style/char_style.rs` — `character_style_no_parent`
  - `src/style/para_style.rs` — `paragraph_style_with_parent`
  - `src/style/list_style.rs` — `list_style_levels_vec`
  - `src/style/table_style_tests.rs:11–24` — `table_style_default_props`
  - `src/style/page_style_tests.rs:22–52` — `new_wraps_a_layout_with_no_display_name`, `page_styles_live_in_the_catalog_keyed_by_id` (tests IndexMap, not this crate), `default_page_styles_catalog_is_empty`
  - `src/layout/header_footer.rs` — both tests; `src/layout/section.rs` — all three (e.g. `two_sections_with_different_page_sizes` asserts A4 ≠ Letter)
  - `src/meta/core.rs` — `custom_property_text` (asserts `prop.name == "Department"` on a literal)
  - `src/document_tests.rs:36–51` — `document_two_sections_different_sizes`
  - default-value spot checks: `char_props_tests::default_has_all_none`, `para_props::default_has_all_none`, `props/drop_cap.rs` both, `props/tab_stop.rs::default_alignment_is_left`, `content/attr.rs::node_attr_default_is_empty`, `meta/core.rs::document_meta_default_is_empty`
- Defect statement: ~30 tests (≈6% of the suite) provide no defect-detection power; they inflate the count and dilute mutation-testing signal.
- Fix: delete, or convert the few with intent (e.g. defaults that encode a spec value, like `default_table_look_matches_word_04a0`, which is *not* in this list because Word's `04A0` default is a real external contract) into documented contract tests.

### F10. `border_none_has_zero_width` does not assert the width
- Crate: loki-doc-model — `src/style/props/border.rs` (test mod, ~line 96) — test `border_none_has_zero_width`
- Category: **B** — Confidence: **HIGH**
- Defect: the name promises zero width; the body asserts only `b.style == BorderStyle::None`. `Border::none()` setting `width: Points::new(3.0)` would pass.
- Evidence:
  ```rust
  let b = Border::none();
  assert_eq!(b.style, BorderStyle::None);   // width never checked
  ```
- Fix: `assert_eq!(b.width.value(), 0.0);` (production `none()` does set 0.0 — verified at src/style/props/border.rs:83–90).

### F11. Footnote-body reject path untested; accept assertion is text-blind
- Crate: loki-doc-model — `tests/loro_nested_revision_tests.rs:134–148` — test `accept_all_resolves_a_change_in_a_footnote_body`
- Category: **C** — Confidence: **HIGH**
- Defect: for the footnote-body container only the *accept* direction is tested, and accepting an insertion leaves the text identical to the input ("keepinstail" before and after), so the text assertion discriminates nothing — only `resolved == 1` and `!has_tracked_changes()` carry weight. The table-cell container gets both directions (`reject_all_removes_a_change_in_a_table_cell` asserts the text visibly changes to "keeptail"); the footnote container has no analogue, so a sweep that reached notes for mark-clearing but failed to apply rejection edits would pass.
- Fix: add `reject_all_removes_a_change_in_a_footnote_body` asserting `note_body_text == "keeptail"`.

### F12. `io/source.rs::builder_chain` — builder echo test
- Crate: loki-doc-model — `src/io/source.rs` (test mod) — test `builder_chain`
- Category: **A** (borderline) — Confidence: **HIGH**
- Defect: asserts that three `with_*` setters stored the values passed; only a field-crosswiring bug could fail it. Marginal value; harmless but counts toward F9's dilution.

### F13. Suite-level: bridge mark/schema wire format has no cross-version fixture
- Crate: loki-doc-model — `tests/loro_bridge_*.rs` (suite-level observation)
- Category: **B** — Confidence: **MEDIUM**
- Defect: nearly all bridge tests are `document_to_loro → loro_to_document` in one process — the classic symmetric round trip. This is substantially mitigated (better than most codebases): `loro_bridge_opaque_tests.rs`/`loro_bridge_table_tests.rs`/`loro_bridge_note_tests.rs` probe the raw Loro containers (`KEY_TYPE`, `KEY_TABLE_CELLS`, `KEY_NOTES` shapes), and `decode_tests.rs` pins the v1 border string. What remains unpinned: the *mark* vocabulary (`MARK_BOLD = "bold"`, color/font/highlight mark encodings) and the opaque-snapshot JSON shape. A coordinated rename (writer + reader + constant) passes every test while orphaning all previously persisted CRDT snapshots/oplogs — the exact hazard for a collaboration server storing oplogs (`loki-server-store`).
- What would settle it: one golden test that imports a checked-in binary Loro snapshot (produced once by the current writer) and asserts `loro_to_document` reconstructs the expected document. That simultaneously covers marks, schema keys, and codecs against accidental format drift.

### F14. `insert_text_at` used as its own oracle in `merged_paragraph_is_editable`
- Crate: loki-doc-model — `tests/loro_para_mark_revision_tests.rs:165–173`
- Category: none (noted, not a defect) — the test deletes at the join and asserts the resulting text; the oracle is the rebuilt document, not the mutation. OK on inspection.

---

## Per-crate stats

- Test files read: **90** (29 in `tests/`, 61 in `src/` — sibling `*_tests.rs` files and inline `#[cfg(test)]` mods)
- Tests examined: **522** (255 integration, 267 unit)
- Findings: **A: 2** (F9 cluster ≈30 tests; F12) · **B: 6** (F1, F2, F3, F4, F7, F10, F13) · **C: 4** (F5, F6, F8, F11)
- Confidence: HIGH ×10, MEDIUM ×2

## Healthy areas

This is, overall, an unusually strong suite — the weak spots above are concentrated in the oldest file (`loro_bridge_tests.rs`) and in trivial constructor tests. Notably good practice observed and verified:

- **Asymmetric probes of the CRDT representation** (`first_block_type`, `table_cell_list_len`, `note_bodies_shape`) break the symmetric-round-trip trap for block structure — opaque vs. native storage is asserted on the Loro side, not just via re-derivation.
- **Deliberate guard inversions**, often with a comment naming the failure mode they discriminate: `ordinary_text_gains_no_line_break`, `a_soft_break_does_not_become_a_hard_break`, `shading_and_named_highlight_do_not_collide`, `unreferenced_style_is_not_touched`, `split_of_an_unstyled_para_copies_no_style_reference`, `paper_entries_are_mutually_distinguishable` (inverts the naming predicate over all pairs), `corner_needs_both_look_flags`, `a_style_with_no_padding_contributes_nothing`, `heading_page_break_attr_survives_a_round_trip` (with a clean control heading).
- **Fixture-validity preconditions** ("establish the phenomenon before removing it") in `page_style_assign_tests.rs` and `delete_removes_the_name_and_keeps_the_geometry` — assertions that the fixture really holds the stale/populated state before the mutation, per the project's evidence rules.
- **Mutation-layer error paths are well covered**: out-of-range indices, invalid byte offsets (multibyte boundary), cross-section merge, cross-container selection, stale offsets rejected *without mutating* (asserted by re-deriving and comparing) — `loro_selection_delete_tests.rs` is exemplary.
- **Concurrency tests** assert convergence on `get_deep_value()` (deliberately stronger than comparing reconstructed `Document`s, and the file says why), plus idempotent re-import and snapshot-into-fresh-doc.
- `IncrementalReader` tests verify both correctness (equality with full rebuild after each mutation) *and* fast-path engagement (`last_update_was_incremental`), including multi-section edits — covering the "instrument speaks about an adjacent quantity" failure.
- `table_banding_tests.rs`, `resolve_tests.rs` (provenance incl. cycles and re-parent guards), `paper_catalog_tests.rs`, and `highlight_rgb_tests.rs` are thorough, discriminating, and test real precedence/boundary logic.
