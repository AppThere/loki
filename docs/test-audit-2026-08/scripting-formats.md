# Test-efficacy audit — scripting/macro-language and lightweight-format crates

Audited 2026-08-23/24, static analysis only. Crates: loki-macro-host, loki-macro-sig,
loki-basic, loki-vba, loki-fountain, loki-markdown. Every test file enumerated and
read in full; for each finding the production code was read to verify the claim
unless marked MEDIUM.

Categories: **A** cannot fail · **B** doesn't test what it claims · **C** not exhaustive
(material gaps only).

---

## loki-basic

### LB-1 · B · HIGH — `feature_refused_for_declared_ffi` never exercises a refusal
- File: `/home/kdesltd/project/loki/loki-basic/tests/interp_tests.rs:233`
- Defect: test name promises "feature refused for declared FFI" but the body never calls the declared function — it runs an unrelated `F` that returns 1.
- Evidence: the body's own comment admits it: `// Calling a Declare'd FFI function is refused (untrappable). Wired in Phase 13's builtin/refusal pass; here we only assert parsing accepts it.` followed by `assert_eq!(run(src, "F", vec![]), Value::Int(1))`.
- The behavior IS properly covered by `refusal_tests.rs:78 ffi_declare_call_refused` (asserts number 1004 + untrappable), so this is a mislabeled duplicate of a parse test. Fixed test: rename to `declare_parses_and_unrelated_procs_still_run`, or delete.

### LB-2 · B · MEDIUM — oversized-allocation test accepts almost any error message
- File: `/home/kdesltd/project/loki/loki-basic/tests/interp_tests.rs:26`
- Defect: `oversized_string_allocation_is_a_runtime_error_not_an_oom` asserts `msg.contains("memory") || msg.contains('7')` — any error whose formatted text contains the digit 7 passes (error 1004, 700, "0.7", a line number…).
- Evidence: `let msg = format!("{err}").to_lowercase(); assert!(msg.contains("memory") || msg.contains('7'), ...)`.
- Fixed test: `assert!(matches!(e, BasicError::Runtime(re) if re.number == 7))` (VBA "Out of memory"), matching the style of `division_by_zero_untrapped_errors`.

### LB-3 · C · HIGH — `MAX_CALL_DEPTH` guard (error 28) has no test anywhere
- File: production `/home/kdesltd/project/loki/loki-basic/src/interp/call.rs:188` (`if self.call_depth >= MAX_CALL_DEPTH { return Err(RuntimeError::new(28, "Out of stack space")); }`)
- Defect: no test in the crate references error 28, "Out of stack", or `MAX_CALL_DEPTH` (verified by grep over `tests/` and all `*tests*` files). Deleting or inverting the guard fails no test; unbounded macro recursion would then blow the real stack (abort, not a trappable error).
- Fixed test: `Sub S(): S: End Sub`-style infinite recursion with a generous fuel budget, asserting `re.number == 28`.

### LB-4 · C · HIGH — panic-freedom suite deliberately avoids the parser's stack-overflow hazard
- File: `/home/kdesltd/project/loki/loki-basic/tests/fuzz_smoke.rs:81` (`moderately_nested_parens_parse_or_error_cleanly`)
- Defect: the parser (`src/parser/`) has no recursion-depth guard (grep for depth/recursion/MAX_ over the module: none), and the panic-freedom test caps nesting at 200 with the comment `// Keep depth modest so the recursive-descent parser does not overflow the test thread's stack` — the suite whose stated purpose is "must return Result (never panic) on adversarial input" is tuned to not reach the one adversarial input class that aborts the process. A hostile macro with a few thousand `(` can crash the host app.
- Fixed state: a depth counter in `parse_expr` returning a syntax error past ~256 levels, plus a test at depth 5&nbsp;000 asserting `is_err()` (not abort). Until the guard exists, the test's name should say "small depths only", per evidence rule 1 (report workarounds plainly).

### LB-5 · C · HIGH — whole statement families have zero execution (and mostly zero parse) tests
- Files: `/home/kdesltd/project/loki/loki-basic/tests/interp_tests.rs`, `tests/parse_tests.rs` vs `src/ast/stmt.rs`
- Defect: these `Stmt` variants are exercised by no test in the crate (grep across `tests/`): `ReDim` / `ReDim Preserve`, `While…Wend`, `Const`, `Static`, `Exit For/Do/Function/Sub`, plain `GoTo` (outside error handling), every `Resume` form, `Error n` (ErrorStmt), `End`/`Stop` (Halt), and `Do … Loop While/Until` **post**-conditions (`do_while_loop` tests a pre-condition; `do_until_loop` in parse_tests only checks the AST variant exists). `ReDim Preserve` appears solely as a fuzz string that must not panic.
- These are interpreter behaviors that can silently regress (e.g. `Preserve` dropping data, `Exit For` breaking the wrong loop, `Resume Next` looping forever).

### LB-6 · C · MEDIUM — operator matrix gaps in `value_tests.rs`
- File: `/home/kdesltd/project/loki/loki-basic/tests/value_tests.rs`
- Defect: `BinOp::Sub`, `Mul`, `Ne`, `Gt`, `Ge`, `Or`, `Xor` (and `Eqv`/`Imp` if implemented) have no direct op-level test; only Add/Div/IntDiv/Mod/Pow/Concat/Eq/Lt/Like/And are covered. Also untested: string-vs-number comparison coercion and `Empty` in comparisons (only `Empty` in arithmetic is tested).
- Fixed test: one table-driven case per remaining operator with a value-and-type assertion.

### LB-7 · B · MEDIUM — op error tests accept any error where a specific number is the contract
- File: `/home/kdesltd/project/loki/loki-basic/tests/value_tests.rs:20,42`
- Defect: `integer_overflow_errors_not_wraps` and `division_by_zero_errors` use bare `.is_err()` although the crate's own error model gives these fixed VBA numbers (6, 11) that the end-to-end tests do assert. A refactor changing overflow to a type-mismatch (13) would pass here.
- Evidence: `assert!(r.is_err(), "Integer+Integer overflow must raise error 6")` — the message names 6, the assertion doesn't check it.
- Fixed test: match the `RuntimeError.number` as `interp_tests::overflow_is_reported` does.

Healthy notes: `refusal_tests.rs` is exemplary (asserts number 1004 **and** untrappable per surface, includes the `On Error Resume Next` bypass attempt and the inversion `ordinary_undefined_call_is_a_normal_error_not_a_refusal`, error 35 trappable). `class_tests`, `host_tests`, `lexer_tests` assert exact values and include both directions of most guards.

---

## loki-macro-host

### MH-1 · B · HIGH — `search_reports_found_or_not` cannot observe the report
- File: `/home/kdesltd/project/loki/loki-macro-host/tests/find_tests.rs:75`
- Defect: the test (and `execute_without_replacement_is_search_only`, `find_via_range_alias_also_works`) declares `Function Main() As Boolean` and claims to test the found/not-found report, but `RunOutcome.result` is `Result<(), MacroRunError>` (`src/runtime.rs:130-134`) — the boolean is discarded by the API. Nothing in the crate asserts `Find.Execute`'s return value; stub `Execute` to always return `True` and the entire find suite still passes.
- Evidence: `let out = run(present, "a haystack without it", vec![]); out.result.expect("clean run"); assert!(out.batch.is_empty());` — no assertion on the searched-for outcome.
- Fixed test: write the boolean into the document (`ActiveDocument.AppendText CStr(Selection.Find.Execute)`) and assert `out.batch.apply_to(...) == "False"` / `"True"` — the pattern `file_tests.rs` already uses for `Err.Number`.

### MH-2 · B · HIGH — `msgbox_is_gated_and_rendered_when_permitted` asserts neither gating nor rendering
- File: `/home/kdesltd/project/loki/loki-macro-host/tests/runtime_tests.rs:203`
- Defect: the body only asserts the run succeeded and the batch is empty; the claim that the dialog reached the backend is explicitly not checked, and a stray `let _ = DocEdit::AppendText(String::new());` pads the test. If `MsgBox` were compiled to a silent no-op (never consulting the backend or the UiDialog grant), the test passes.
- Evidence: `// (Recovered via the batch being empty; backend inspection needs into_parts, exercised in the unit tests — here we just assert the run succeeded.)` — but no unit test in this crate inspects a `TestBackend.dialogs` count for the granted path either (the loki-basic `host_tests` do, against the interpreter's own mock, not through `MacroRuntime`'s broker gating).
- Fixed test: give `TestBackend` a dialog log (it already has the field type in runtime_tests' sibling backends) and assert `dialogs.len() == 1` after the permitted run, and `0` for a denied run.

### MH-3 · B · HIGH — `ids_are_stable_and_unique` asserts only non-emptiness
- File: `/home/kdesltd/project/loki/loki-macro-host/src/capability_tests.rs:17`
- Defect: the name promises stability and uniqueness; the body is `for cap in Capability::ALL { assert!(!cap.id().is_empty()); }`. Uniqueness lives in the *previous* test; stability is asserted nowhere — yet ids key persisted state (trust-store JSON grants survive reload via serde), so renaming an id silently orphans users' stored grants and no test notices.
- Fixed test: golden assertion of the exact id list (e.g. `assert_eq!(ids, ["clipboard_read", "doc_read", ...])`), which is the "one fact, one derivation" anchor for the on-disk format.

### MH-4 · C · HIGH — hex `decode32` invalid-character path is unreachable by the tests
- File: `/home/kdesltd/project/loki/loki-macro-host/src/trust/hex.rs:56` (`rejects_bad_input`)
- Defect: every rejection case is caught by the **length** guard (`""`, `"zz"` (len 2), 63, 65 chars); no case is 64 chars long with a non-hex character, so the `to_digit(16)?` error branch (line 33-34) is never exercised. Additionally `roundtrip` is a symmetric encode→decode with no fixed vector — a consistent nibble-order swap in both functions passes, and would corrupt interop with any store written by a correct implementation.
- Fixed test: `assert_eq!(decode32(&("0".repeat(63) + "g")), None)` and a golden pair `encode(&[0xDE, 0xAD, ...]) == "dead…"`.

### MH-5 · C · MEDIUM — dialog rate limiter's refill arm untested and untestable
- File: `/home/kdesltd/project/loki/loki-macro-host/src/dialog_rate.rs:47`
- Defect: the single test drains the initial burst and asserts refusal; nothing tests that tokens *refill* with elapsed time. Setting `REFILL_PER_SEC = 0.0` (macro permanently suspended after 5 dialogs — a user-visible bug) fails no test. `Instant::now()` is hardwired, so the refill arm cannot be tested without sleeping.
- Fixed state: inject a clock (a `now: fn() -> Instant` or trait), then assert one token returns after ~2 s of simulated elapse and that the bucket caps at CAPACITY.

Healthy notes: this crate's suites are the strongest of the six. `broker_tests` and `net_policy_tests` invert every guard (refused-despite-grant, deny-does-not-record, UDF-denies-all, redirect deny/bad/relative, cap-boundary body reads); `trust/store_tests` covers T10 in both directions plus legacy-record provenance defaults; `file_tests` (integration) asserts effect logs, negative reaches ("picker must not be raised"), and two genuine regression inversions (failed-close retry, idempotent close); `http_tests` (src) documents two real regressions (default-port, IDN) with discriminating assertions.

---

## loki-macro-sig

### MS-1 · C · MEDIUM — the c14n transform is never exercised on non-canonical input end-to-end
- Files: `/home/kdesltd/project/loki/loki-macro-sig/src/verify_odf_tests.rs` (all), `src/xml_c14n_tests.rs`
- Defect: every XMLDSig fixture authors `SignedInfo` **already in canonical form** ("so the signed octets do not depend on our own canonicaliser") — so the verify pipeline (`verify_odf.rs:122` does call `canonicalize`) only ever canonicalizes input that is a fixed point. A canonicalizer bug that mangles pretty-printed/whitespace-bearing XML (what LibreOffice actually emits) is invisible to the end-to-end suite; the unit tests cover the rules but on hand-built trees, not parsed real documents.
- Mitigation exists and is honestly marked (`TODO(8A.4-corpus)` real-signer interop). Fixed test until the corpus lands: one verify-path fixture whose `SignedInfo` is signed over its canonical form but *embedded* with indentation/attribute reordering, asserting ValidUntrusted.

### MS-2 · C · MEDIUM — SHA-384/512 digest correctness asserted by length only
- File: `/home/kdesltd/project/loki/loki-macro-sig/src/verify_crypto_tests.rs:46`
- Defect: `digest_produces_expected_lengths` checks `.len()` only. Sha256/Sha1 are anchored by the end-to-end CMS tests, but Sha384/Sha512 appear nowhere else — a stub returning `[0u8; 48]` passes.
- Fixed test: compare against NIST test-vector hex for one input per algorithm.

### MS-3 · C · MEDIUM — not-yet-valid certificate path untested
- File: production `/home/kdesltd/project/loki/loki-macro-sig/src/verify.rs:150` (`t >= info.not_before`); tests cover only expiry.
- Defect: no fixture has `not_before` in the future, so inverting the `>=` fails no test. (A forged "post-dated" cert should read CertificateExpired/untrusted, not valid.)
- Fixed test: an `expired=false` cert with `not_before = 2033`, asserting the verdict is not NotPinned-valid.

Healthy notes: exceptional suite overall. Fixtures are real crypto built in-process (fresh RSA/P-256 keys, real CMS/XMLDSig), never verifier-derived goldens; every failure verdict is asserted as the *specific* `InvalidReason`/`UntrustedReason`; the timestamp suite includes the transplant attack (`unbound_timestamp_is_ignored`) and both rescue polarities; `verify_odf_tests::signature_not_covering_a_required_part_is_content_mismatch` is a textbook coverage inversion.

---

## loki-vba

### MV-1 · B · HIGH — `decompress_bomb_guard_bounds_output` asserts nothing and cannot reach the guard
- File: `/home/kdesltd/project/loki/loki-vba/tests/fuzz_smoke.rs:31`
- Defect: the test name and doc claim the 4096-byte chunk-expansion guard is exercised; the body is `let _ = decompress(&input);` with no assertion, and its own comment concedes the fixture errors *before* the guard can act: `// with no prior output the first copy is invalid, so this must error`. Deleting `CHUNK_LIMIT` check (`decompress.rs:85-87`) or `MAX_OUTPUT` (`:34-36`) fails no test in the crate.
- Fixed test: seed the chunk with one literal then max-length overlapping copy tokens (valid offsets) so expansion is real, and assert `Err(VbaError::Compression(_))`; separately craft a many-chunk container crossing MAX_OUTPUT and assert `Err(VbaError::TooLarge)`.

### MV-2 · C · MEDIUM — decompressor error branches lack asserting tests, and the two that exist accept any error
- Files: `/home/kdesltd/project/loki/loki-vba/src/decompress.rs:154-161` (tests), production branches at lines 49, 55, 95, 106
- Defect: only "missing signature" and "truncated header" are asserted, both via bare `.is_err()`. "bad chunk signature", "truncated chunk data", "truncated copy token", "copy offset precedes chunk start" appear only in the no-assertion fuzz smoke. Swapping any of these to a silent `return Ok(())` (truncation-tolerant parsing — a classic malware-evasion vector in OVBA parsers) fails no test.
- Fixed test: one fixture per branch asserting `matches!(e, VbaError::Compression(_))` (ideally on message or a structured kind).

### MV-3 · C · MEDIUM — raw (uncompressed) chunk branch never executed
- File: production `/home/kdesltd/project/loki/loki-vba/src/decompress.rs:59-65` (`compressed == false` → verbatim copy)
- Defect: every fixture in the crate sets header bit 0x8000; the raw-chunk copy path (which real Office files do emit for incompressible data) has zero coverage. Note the crate's own compressor never emits raw chunks, so round-trip tests structurally cannot cover it.
- Fixed test: hand-build `[0x01, header(raw, len)] + data` and assert the bytes come back verbatim.

Healthy notes: the compressor round-trip suite is *not* a naive symmetric round-trip — it is anchored by a byte-exact golden vector (`abc_matches_the_decompressor_test_vector`) and an independent structural walk of chunk headers (`emitted_chunks_are_structurally_valid`), exactly the discipline rule 2 asks for. `write_tests` asserts stripped p-code at the raw-CFB level and includes the uncodepageable-character refusal as a typed error.

---

## loki-fountain

### MF-1 · C · HIGH — `===` page breaks and `>centered<` alignment never verified past classification
- Files: `/home/kdesltd/project/loki/loki-fountain/src/classify_tests.rs:56,45`; `src/emit.rs:22-40`; `tests/import.rs`
- Defect: classify tests stop at `Element::PageBreak` / `Element::CenteredAction`; the *emission* those elements exist for — `page_break_before` from a `===` and `alignment: Center` on centered action (`emit.rs:34-39`) — is asserted by no test. The only page-break assertion in `tests/import.rs` is the title-page `break_first` path. Stub `Element::PageBreak => { continue; }` (dropping `pending_break = true`) or delete the `Center` alignment and every test passes.
- Fixed test: run `parse_str("One.\n\n===\n\nTwo.")` and assert the "Two." paragraph's `page_break_before == Some(true)`; a `>centered<` fixture asserting `alignment == Some(Center)`.

### MF-2 · C · MEDIUM — minor untested surfaces: `~` lyrics, CRLF sources, invalid-UTF-8 error
- Files: `/home/kdesltd/project/loki/loki-fountain/src/classify.rs:123` (lyric marker), `src/title.rs:28-31` (CRLF blank-line split), `src/lib.rs:40` (`FountainError::Utf8`)
- Defect: the forced-marker test covers `!` `.` `>` `@` but not `~`; no test feeds a CRLF file (the title-page terminator has a dedicated `\r\n\r\n` branch that is dead in tests); no test imports non-UTF-8 bytes to pin the typed `Utf8` error (vs a panic or lossy decode).

Healthy notes: `classify_tests` is a model of guard inversion — every context rule is tested with the input that must *not* match it (heading without preceding blank → Action, `CUT TO:` followed by content → cue, lone caps line → action). The import test's style-id sweep against `REQUIRED_STYLE_IDS` mechanically keeps importer and template in step.

---

## loki-markdown

### MD-1 · B · HIGH — `inline_wrappers_nest_and_links_carry_targets`: no nesting in the fixture, wrapper bodies unchecked
- File: `/home/kdesltd/project/loki/loki-markdown/tests/import.rs:121`
- Defect: the fixture line has `*emphasis*`, `**strength**`, `` `code` `` and a link as **siblings** — nothing nests — and the assertions are `matches!(i, Inline::Emph(_))` / `Inline::Strong(_)`, which pass even if the wrapper bodies are empty. Gut `pop_wrap` to discard `body` (`convert_flush.rs:31-43`) and only the link assertion fails; make the fixture's claim of nesting true nowhere.
- Fixed test: a fixture like `**bold with *italic* inside** and [a *styled* link](u)`, asserting the inner structure (`Strong([Str, Emph([Str]), Str])`).

### MD-2 · C · HIGH — table content is never asserted, only the table's existence
- File: `/home/kdesltd/project/loki/loki-markdown/tests/import.rs:77`; production `src/convert_flush.rs:113-128` (`flush_table`)
- Defect: `assert!(blocks.iter().any(|b| matches!(b, Block::Table(_))))` is the entire table coverage. The row/column/cell placement loop — the part that can transpose or drop cells (and the `cols` max-width padding for ragged rows) — is fully mutation-survivable: replace the loop body with nothing and the test passes (an empty grid is still a `Block::Table`).
- Fixed test: assert dimensions and the cell inlines (`bodies[0].body_rows[1].cells[0] == Para([Str("1")])`) for the existing `| A | B |` fixture, plus one ragged-row case.

### MD-3 · C · MEDIUM — documented behaviors and event arms with zero coverage
- Files: `/home/kdesltd/project/loki/loki-markdown/src/convert.rs`, `tests/import.rs`
- Defect (each is an implemented, reachable arm with no test): images (`Tag::Image` → `Inline::Image` with alt/title), strikethrough, task-list markers (☑/☐ text), `SoftBreak`/`HardBreak`, heading levels 2–6 (only H1 in fixture), nested blockquotes (depth counter), and the crate-doc's own promise that a loose item's extra paragraphs become plain paragraphs after the item (`item_pending` precedence in `flush_paragraph`) — the "deliberate simplification" the module header advertises is untested.

Healthy notes: the list test is precise (exact `(list_id, level)` sequence including the nested level bump) and verifies the seeded style catalog — a real export-time invariant.

---

## Per-crate statistics

| Crate | Test files read | Tests examined (approx) | A | B | C | Total findings |
|---|---|---|---|---|---|---|
| loki-macro-host | 20 | ~159 | 0 | 3 | 2 | 5 |
| loki-macro-sig | 11 | ~61 | 0 | 0 | 3 | 3 |
| loki-basic | 13 | ~120 | 0 | 3 | 4 | 7 |
| loki-vba | 6 | ~29 | 0 | 1 | 2 | 3 |
| loki-fountain | 5 | 14 | 0 | 0 | 2 | 2 |
| loki-markdown | 1 | 3 | 0 | 1 | 2 | 3 |
| **Total** | **56** | **~386** | **0** | **8** | **15** | **23** |

(Confidence: 18 HIGH, 5 MEDIUM. No category-A findings — every test in these crates
contains at least one live assertion against production output; the failure modes here
are misdirected or missing assertions, not tautologies.)

## Healthy areas

- **Security-posture suites are the strongest in the workspace**: loki-basic
  `refusal_tests` (specific error number + untrappability + the `On Error` bypass
  inversion + the not-everything-is-refused inversion), loki-macro-host broker/trust/
  file/http suites (negative reaches asserted via effect logs; T10 tested in both
  directions; two documented regressions pinned with discriminating tests), and
  loki-macro-sig's end-to-end crypto fixtures with per-reason verdict assertions
  including the timestamp-transplant attack.
- **loki-vba's compressor avoids the symmetric-round-trip trap** by anchoring with a
  byte-exact golden vector and an independent structural validation of the emitted
  chunk stream.
- **Golden values are essentially never computed by the code under test** anywhere in
  these six crates — fixtures are hand-derived bytes/DER/XML or built by independent
  libraries. The classic interpreter-tests-its-own-output failure mode is absent.
- The weak spots cluster in three shapes: (1) guards that exist in production but no
  test can kill (call depth, bomb caps, hex char validation, rate refill), (2) names
  that promise an assertion the API cannot even observe (`Find.Execute`'s boolean,
  MsgBox rendering, capability-id stability), and (3) breadth gaps in the newest,
  smallest crates (markdown/fountain emission layer, BASIC statement tail).
