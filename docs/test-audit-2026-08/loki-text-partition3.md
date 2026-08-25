# loki-text — partition 3 findings (macro bridge/apply/run, defaults, style target, dialog walk, wheel zoom, spell UI; 23 files, 154 tests)

### `editor_macro_bridge_tests.rs:259` — `a_gone_ui_degrades_to_deny` — Category B — HIGH
**Defect:** The test claims to cover the closed-channel→Deny degradation but its "UI" explicitly answers `Deny`, so the actual gone-UI code paths are never executed.
**Evidence:** Policy is `|_req| Some(UiReply::Grant(GrantScope::Deny))`; the comment admits it "models the closed-channel path's effect". Production `ask()` has two real degradation branches — `if self.req_tx.unbounded_send(pending).is_err() { return None; }` and `reply_rx.recv().ok()` (editor_macro_bridge.rs:144–147) — and no test in the file ever closes the channel or drops a `PendingPrompt` unanswered, so both branches could be replaced with a panic/permanent block and every test still passes.
**Fix:** Drop the receiver (or drop the `PendingPrompt` without answering) mid-run and assert the worker still finishes with a denied outcome instead of wedging.

### `editor_macro_apply_tests.rs:77` — `set_text_preserves_block_zero_as_an_editable_paragraph` — Category A — HIGH
**Defect:** The property the test is named for is asserted by a discarded function call, which cannot fail.
**Evidence:** `let _ = get_block_style_name(&loro, 0);` — production `get_block_style_name` returns a plain `String` and returns `""` for an unreadable/out-of-range block (loro_mutation/style.rs:23–27), so the call succeeds for *any* post-SetText state; only the text assertion is real.
**Fix:** Assert the returned style name is non-empty (or equals the expected default), i.e. `assert!(!get_block_style_name(&loro, 0).is_empty())`.

### `editor_macro_apply_tests.rs:45` — `set_text_collapses_the_body_to_one_paragraph` — Category B — HIGH
**Defect:** "Trailing blocks are gone" is asserted via a getter that returns `""` both for a deleted block and for a surviving-but-emptied block, so a SetText that empties instead of deletes passes.
**Evidence:** `assert_eq!(get_block_text(&loro, 1), "");` with the comment "(out of range)". Mutation check: replacing the production `delete_block` loop (editor_macro_apply.rs:97–99) with per-block `delete_text` leaves 3 empty paragraphs; this test, and `set_then_append_operates_on_the_collapsed_body` (line 54, same idiom), still pass because block-count is tracked as `blocks = 1` internally and appends land on block 0 either way.
**Fix:** Rebuild the document (`loro_to_document`) and assert `sections[0].blocks.len() == 1`.

### `editor_macro_run_tests.rs:91` — `apply_and_report_applies_a_batch_as_one_undo_entry` — Category B — HIGH
**Defect:** The test never asserts undo granularity, the one property its name (and macro spec §6.2) claims.
**Evidence:** Assertions are `report.ok && report.applied`, `report.message == "done-edited"`, and the final text. The "one undo entry" mechanism is the single `loro.commit()` in `apply_edit_batch` (editor_macro_apply.rs:47); committing once per edit inside `apply_batch_ops` would pass every existing test.
**Fix:** Run a multi-edit batch under a Loro `UndoManager`, perform one undo, and assert the document is back to its pre-run text.

### `editor_defaults_tests.rs:127` — `a_seeded_document_holds_its_geometry_not_a_reference_to_the_settings` — Category A — HIGH
**Defect:** The test's second half asserts a field equals the value the test itself just assigned to it — a tautology; the first half duplicates `a_recorded_page_size_and_margins_reach_the_new_document`.
**Evidence:** `reopened.sections[0].layout.page_size = seeded.clone(); assert_eq!(reopened.sections[0].layout.page_size, seeded);` — no production code runs between assignment and assertion (the promised "round trip through the model" is a plain field write).
**Fix:** Seed a document, then call `apply_document_defaults` on it *again* with different defaults while modelling the opened-file path (the Blank-arm-only seam), or delete the test — the property as stated is not testable at this layer.

### `editor_macro_run_tests.rs:79` — `make_run_request_reads_title_body_and_grants` — Category B — MEDIUM
**Defect:** The name claims the title is read but the test never asserts `req.title` — only text and grants.
**Evidence:** Production `read_document` returns `(title, text)` and `RunRequest::new(title, text, RUN_FUEL)` (editor_macro_run.rs:70–72); the test asserts only `req.text == "Hello\nworld"` and `req.grants.contains(...)`. Returning `String::new()` for the title would pass.
**Fix:** Set a known document title in the fixture and assert it appears on the request.

### `editor_macro_run.rs:79` — network-policy gating (no test) — Category C — MEDIUM
**Defect:** The `MACRO_NET_COMPILED && svc.network_enabled(payload)` gate in `make_run_request` has no test at this layer in either polarity.
**Evidence:** `if loki_macro_host::MACRO_NET_COMPILED && svc.network_enabled(payload) { request = request.with_network(NetworkPolicy::enabled()); }` — the bridge tests enable network by hand via `drive_net`, bypassing this gate; deleting the `network_enabled` check (always-on network) fails no loki-text test.
**Fix:** With the `macro-net` feature, assert a payload without the runtime opt-in yields a request whose network policy is disabled, and one with the opt-in yields enabled.

### `editor_style_target.rs:51` — empty-key guard (tests: `editor_style_target_tests.rs:151`) — Category C — HIGH
**Defect:** The guard that prevents seeding a style under an empty id lives in untested `dialog_style_target`, and the unit test explicitly pins that the tested layer *would* invent one.
**Evidence:** Test `resolution_never_invents_a_target_for_no_block` asserts `resolve(&mut c, "", None)` returns `Some(("", seeded=true))` and notes "`dialog_style_target` guards the empty key". Production: `if key.is_empty() { return None; }` (editor_style_target.rs:51) — no test calls `dialog_style_target`, so deleting the guard fails nothing.
**Fix:** A test driving `dialog_style_target` against a Loro doc with the cursor on no block, asserting `None` and that no `""`-id style was persisted into the catalog.

### `dialog_walk_tests.rs:108` — `deeply_nested_inlines_are_visited` — Category B — MEDIUM
**Defect:** The doc comment claims the fixture is "an image inside an underlined run inside a link", but the body nests `Str` inside `Strikeout` inside `Underline` — no `Image` or `Link` variant is exercised anywhere in the file.
**Evidence:** `Inline::Underline(vec![Inline::Strikeout(vec![Inline::Str("deep")])])` — a walker that skipped `Inline::Link`'s children entirely would pass this suite.
**Fix:** Nest through `Inline::Link` (and an `Inline::Image` leaf) as the comment describes, asserting both are visited.

### `editor_macro_bridge_tests.rs:177` — `stop_during_a_prompt_unblocks_the_worker` — Category B — MEDIUM
**Defect:** The "unblock" claim is satisfied because the UI answers the prompt, not because Stop unblocked anything — the worker is never actually left blocked.
**Evidence:** The policy trips cancel then immediately returns `Some(UiReply::Grant(GrantScope::Deny))`; `ask()` checks cancel only *before* sending (editor_macro_bridge.rs:136), and `reply_rx.recv()` has no cancel-during-wait path, so removing all cancel-awareness from `ask()` leaves this test green.
**Fix:** Assert what is actually guaranteed (cancel + reply ends the run denied) under an honest name, and cover real unblocking via the channel-close path (see the `a_gone_ui_degrades_to_deny` fix).

Partition stats: 23 files read, 154 tests examined, A=2 B=6 C=2.

Healthy areas: this partition is well above average. `editor_wheel_zoom_tests.rs` (NaN/∞ inputs, floor-not-always-taken polarity, full-range sweep), `editor_highlight_color_tests.rs` (route inversion, stale-mark cross-route checks, round-trip into the *distinct* underlying props rather than symmetric read-back), `editor_spell_place_tests.rs` (anti-constant polarity test, scroll-term sweep), `editor_spell_rows_tests.rs`, `editor_ribbon_layout_tests.rs`, `font_family_list_tests.rs`, `editor_doc_colors_tests.rs`, and `editor_seed_publish_tests.rs` (independently-known word count per L08-028) all show deliberate inversion and boundary coverage; the macro-host bridge/apply cluster is where the real gaps concentrate.
