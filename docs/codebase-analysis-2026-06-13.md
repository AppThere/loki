# Codebase Analysis & Improvement Proposals — 2026-06-13

This is a fresh, current-state pass over the workspace (17 crates, ~60k LOC of
`src` + tests). It complements [`audit-2026-06-10.md`](audit-2026-06-10.md) by
**verifying which of that audit's findings are now closed** and proposing the
next round of fixes, features, performance work, quality cleanups, and test
coverage.

Method: structural exploration via the `code-review-graph` MCP, targeted source
verification of each claim, and `cargo check`/`clippy`/`test` where the crate
builds without a GPU. `cargo check --workspace` passes (exit 0) at the time of
writing.

---

## 0. What the 06-10 audit got right — and what is already fixed

Most of the high-severity audit items have since been resolved. Verified in the
current tree:

| Audit item | Status (verified) | Evidence |
|---|---|---|
| **S1** ZIP decompression limits | **Fixed** | `loki-opc/src/zip/limits.rs` (`read_entry_capped`, per-entry + aggregate budgets); `loki-odf/src/limits.rs` |
| **S2** ODS repeat amplification | **Fixed** | `loki-odf/src/ods/import.rs` clamps to `MAX_SHEET_ROWS/COLS`, `MAX_MATERIALIZED_*`, `saturating_add` |
| **S3** ODT allocation bombs | **Fixed** | `MAX_REPEATED_SPACES`, `MAX_TABLE_COLUMNS` clamps in `odt/mapper/document.rs` |
| **S4** ODT unbounded recursion | **Fixed** | `MAX_NESTING_DEPTH` guard in `odt/reader/document.rs:633` |
| **R2** `from_hex` multibyte panic | **Fixed** | ASCII guard at `loki-primitives/src/color/document.rs:87` + regression test |
| **R2** style parent-chain cycles | **Fixed** | `MAX_STYLE_CHAIN_DEPTH` caps in `style/catalog.rs` and `loki-layout/src/resolve.rs:255` |
| **C1** lists/tables → horizontal rule | **Fixed** | opaque-snapshot fallback in `loro_bridge/write.rs:77` (`write_opaque_block`) |
| Memory #1/#4 (hot-tier, shaping cache) | **Fixed** | per `memory-audit-2026-06-12.md` + recent commits |

**Takeaway:** the malicious-document DoS surface and the worst CRDT data-loss
path are closed. Remaining opportunities are concentrated in *functionality
breadth*, *quality/consistency*, and *test coverage* rather than crash/security.

---

## 1. Fixes (correctness)

Verified still-open items worth prioritising:

- **F1 — Presentation editor is a hardcoded demo (HIGH).** `loki-presentation/`
  `src/routes/editor/editor_inner.rs:40+` still builds a fixed `vec![Slide{…}]`
  of marketing slides; there is no `.pptx`/`.odp` load path and no save. Opening
  a real file shows fake content and silently discards edits. Either wire a real
  import/save pipeline or gate the editor behind a clear "preview only" state so
  users cannot lose work.
- **C4 — byte vs Unicode cursor offsets (MEDIUM).** Re-verify
  `loro_bridge/mod.rs` cursor calls still pass UTF-8 byte offsets to Loro's
  Unicode-position API; if so, convert with
  `text.convert_pos(byte, PosType::Bytes, PosType::Unicode)`. Affects all
  non-ASCII editing.
- **R3 fidelity bugs** (multi-tab stop positions; image transform ignoring
  layout rect size/DPR; keep-with-next chains dropping inline images/footnotes)
  — confirm against current `loki-layout`/`loki-vello` before scheduling; some
  may have moved with the recent layout refactor.

## 2. Feature enhancements

- **Save As / Ctrl+S in `loki-text`.** The spreadsheet has `pick_file_to_save`;
  `loki-text` does not, so untitled documents can never be persisted. Lift the
  save-picker into shared code and bind `Ctrl/Cmd+S`.
- **Selection-aware typing & clipboard.** Typing/Backspace over an active
  selection should replace it; add cut/copy/paste. Currently insertion happens
  at the focus without deleting the selection (also noted for the reflow view in
  `fidelity-status.md`).
- **Spreadsheet formula engine.** Only `SUM` and `+`/`-` evaluate today; the
  grid is hardcoded to `A1:J30`. A minimal function set (`AVERAGE`, `MIN`,
  `MAX`, `IF`, cell ranges) and a dynamic used-range would make it usable.
- **`loki-opc` as a standalone crate.** The crate is MIT-licensed and slated for
  public release. With the documentation cleanup in this branch it is close to
  publishable; the remaining gaps are digital-signature support (§10, currently
  `DigitalSignaturesNotSupported`) and a published API doc pass.

## 3. Performance

Recent commits already landed the big wins (single canonical layout pass,
incremental Loro→Document reconstruction, shaping cache, tier-0 retier on load).
Remaining items from `memory-audit-2026-06-12.md` that are still *Recommended*:

- **Page virtualization** — every page is a mounted tile holding a texture; mount
  only near-viewport pages.
- **Inactive-tab session retention** — inactive tabs keep full layout + Loro +
  undo resident; serialise/evict cold tabs.
- **Shared font byte cache across tiles** — per-tile `FontDataCache` duplicates
  interned font bytes.
- **Loro oplog compaction** — oplog grows with history and never compacts.

## 4. Quality / conventions

- **AI-generated "word-salad" documentation (NEW — fixed for `loki-opc` in this
  branch).** The entire `loki-opc` crate shipped with machine-generated doc
  comments — run-on gerund/adverb chains that conveyed no information (e.g.
  *"Reads packages sequentially organizing files correctly instantiating
  metadata components…"*). ~40 doc/comment sites across all modules plus every
  `OpcError`/`DeviationWarning`/`CoreProperties` field were rewritten to
  accurate, concise docs, and two pre-existing broken intra-doc links were
  fixed so `cargo doc` is clean. **Recommendation:** sweep the other crates for
  the same pattern (a quick heuristic: doc lines >120 chars with ≥4 `-ing`
  words and ≥2 of {natively, cleanly, seamlessly, robustly, …}).
- **300-line file-ceiling debt.** ~40 `.rs` files exceed the 300-line ceiling
  (worst: `loki-layout/src/flow.rs` 1484, `loki-odf/src/odt/reader/styles.rs`
  1441, `…/reader/document.rs` 1428, `loki-layout/src/para.rs` 1306). The
  CLAUDE.md "known tech debt" table lists only two of these and is stale; either
  schedule a split pass or update the table to reflect reality.
- **Hardcoded user-visible strings (i18n).** `loki-spreadsheet` and
  `loki-presentation` bypass `loki-i18n` wholesale; `loki-text` style editor and
  save errors have literals. Route through `fl!()`.
- **Crate-level clippy allows** in `loki-odf`/`loki-ooxml` `lib.rs` contradict
  the "never crate level" rule; narrow them to the offending items.
- **Tracked junk at repo root** (`scratch.rs`, `diff_flow.txt`, `iris*.png`,
  `*_log*.txt`, `test_output*`, `output.png`) should be removed and
  `.gitignore`d.
- **Pre-existing `collapsible_if`** in `loki-opc/src/compat/content_types.rs`
  surfaces only under `--all-features` with clippy ≥1.94; collapse the nested
  `if let`/`if` (left untouched here as out-of-scope for a docs pass).

## 5. Test coverage

718 `#[test]` functions today, but coverage is very uneven:

| Crate | Tests | Note |
|---|---|---|
| loki-odf / loki-ooxml / loki-doc-model / loki-layout | 176 / 160 / 132 / 124 | strong |
| **loki-opc** | 10 | thin for a crate going public — add round-trip + deviation-shim + zip-bomb-limit tests |
| **loki-spreadsheet, loki-presentation** | **0** | no tests on the app/editor logic |
| **loki-sheet-model, loki-i18n, loki-fonts** | **0** | pure logic crates, cheap to cover |
| **appthere-ui, appthere-canvas** | **0** | token/cache logic is unit-testable |

Highest-leverage, lowest-cost additions:

1. `loki-opc`: assert the zip-bomb budgets actually trigger
   (`EntryTooLarge`/`PackageTooLarge`), the compat shims emit the right
   `DeviationWarning`, and `resolve_relative_reference` handles `../` targets.
2. `loki-i18n`: the `en-US-posix` fallback bug (raw keys rendered) is a
   one-test regression guard.
3. `loki-sheet-model`: cell/range model invariants.

---

## Recommended priority order

1. **F1 presentation data-loss** + **Save As/Ctrl+S in loki-text** — users can
   still lose work.
2. **C4 Unicode cursor offsets** — silent corruption for non-ASCII editing.
3. **Test coverage for the zero-test crates** and `loki-opc` budgets — lock in
   the security fixes before they regress.
4. **Quality sweep**: word-salad docs in remaining crates, 300-line splits,
   i18n the app crates, delete root junk.
5. **Performance**: page virtualization + inactive-tab eviction (largest
   remaining memory wins).
