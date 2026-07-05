<!--
SPDX-License-Identifier: Apache-2.0
-->

# Deferred-Features Audit — AppThere Loki (2026-07-04)

| | |
|---|---|
| **Status** | Audit complete; **no code changes**. Inventory + verification of every documented deferral. |
| **Scope** | First-party `loki-*` / `appthere-*` source + all `docs/` (specs, ADRs, prior audits, `CLAUDE.md`). Vendored `patches/` and `/target/` excluded. |
| **Method** | Five parallel verification passes over (1) Rust `TODO(topic)` annotations, (2) `COMPAT` workarounds + dependency patches, (3) Spec 01–06 docs, (4) prior audit docs, (5) `fidelity-status.md` / tech-debt / MVP scope / format ADRs. **Every deferral was cross-checked against the current code** — the goal was not just to list them but to catch cases where a doc still says "deferred" but the code already does it. |
| **Branch** | `claude/adr-docs-setup-ogwz5a` |
| **Supersedes/extends** | [`docs/adr/spec-01-todo-compat-inventory.md`](adr/spec-01-todo-compat-inventory.md) (TODO/COMPAT catalogue, 2026-06-28) — still current and CI-enforced; this report adds verification + the doc-level deferrals it did not cover. |

Counts at a glance: **~70 `TODO(topic)`** annotations (34 distinct deferrals), **~70 `COMPAT`** workarounds, **6** dependency patches, **35** files over the 300-line ceiling, and dozens of spec/audit/registry deferrals. The signal worth acting on first is **§1**.

---

## 1. Stale documentation — the code has moved on (ACTION: fix the docs)

These are the audit's most important findings: places where a document still records something as **deferred / recommended / open / unsupported**, but verification shows the code **already implements it**. Correcting these prevents wasted "let's finally do X" effort and stops the docs from lying.

| # | Doc claim (source) | Reality in code | Recommended fix |
|---|---|---|---|
| S-1 | `COMPAT(dioxus-native): position: absolute is unsupported in Blitz` (~5 sites: `appthere-ui/.../ribbon/group.rs:41`, `loki-text/.../editor_inner.rs:679,696`, `loki-renderer/src/page_tile.rs:105`, `.../editor_style_editor/form_font.rs:33`) | `CLAUDE.md` "Confirmed CSS properties" (2026-06-28, runtime-verified) states block-level `position: absolute` lays out, paints above the wgpu canvas, and hit-tests; the spelling context menu already relies on it. | Re-evaluate the flex/auto-margin fallbacks at those sites; update or drop the COMPAT notes. **Caveat:** absolute *inside an inline formatting context* is still unverified, and `position: fixed` genuinely still collapses to `absolute`. |
| S-2 | `TODO(editing)` — `ParagraphStyle::next_style_id` "used by split_block to determine the style of the newly created paragraph after Enter" (`loki-doc-model/src/style/para_style.rs:53`) | Fully wired: `editor_keydown_ctrl.rs:200-224` resolves `next_style_id` and calls `set_block_style` on the new block. | Remove the stale `TODO(editing)`. |
| S-3 | `TODO(super-sub)` "only font-size reduction applied" (`loki-layout/src/para.rs:177,870`) | `para_emit.rs:104,160` now also apply a manual `va_offset` vertical shift + per-glyph `w:position` `baseline_shift`; super/subscript are actually raised/lowered. | Reword: only the *native Parley `BaselineShift` API* is missing, not the effect. |
| S-4 | memory-audit **Finding 2** "Virtualize page tiles" — *Recommended (not implemented)* (`docs/memory-audit-2026-06-12.md`) | Implemented: `loki-renderer/src/virtualize.rs` `visible_window`; `document_view.rs:290` mounts `PageTile`s only within the viewport window, placeholders elsewhere. | Mark Finding 2 **Fixed**. |
| S-5 | audit-2026-06-10 **C1** "lists/tables/figures collapse to `HorizontalRule` on first CRDT write" — *Critical open* | `loro_bridge/write.rs:22` writes a native `Block::Table` (and siblings); fidelity gap #1 resolved. | Mark C1 **resolved**. |
| S-6 | audit-2026-06-10 **S1/S2/S3/S4** (zip-bomb / ODS repeat / ODT attribute amplification / unbounded ODT recursion) — *open* | Mitigated: entry+aggregate caps, `MAX_MATERIALIZED_*`, checked arithmetic, and `MAX_NESTING_DEPTH` guard (`odt/reader/document.rs:22`); audit-2026-06 "Verified mitigations" confirms. | Mark S1–S4 **resolved**. |
| S-7 | fidelity-audit **gap #14** "horizontal text scale (`w:w`) not applied" — *OPEN* | `loki-layout/src/resolve.rs:390` forwards `.scale` to `StyleSpan`. | Mark gap #14 **resolved**. |
| S-8 | `CLAUDE.md` Loro round-trip table: "DocumentMeta/DublinCore … export drops the extended Dublin Core fields" | `fidelity-status.md` §1/§9 state metadata now round-trips *and* writes back to DOCX + ODT. (Doc-vs-doc contradiction.) | Reconcile — update the `CLAUDE.md` row to match `fidelity-status.md`. |
| S-9 | `CLAUDE.md` "worst offenders" ceiling table: `flow.rs` 1612 / `para.rs` 1278 / `odt/reader/styles.rs` 1441 | `file-ceiling-baseline.txt` records `para.rs` 1979 / `flow.rs` 1953 / `styles.rs` 1554 — the files **grew**. The count (35) is right; the sizes are stale. | Regenerate the offenders table from the baseline. |

> **Note on "resolved-as-decision" (Spec 02):** several Spec 02 items are recorded with a "✅ Resolved" *decision* but were never built — **Gelasio font not bundled** (`loki-fonts/fonts/` has no Gelasio face), **vendored schemas absent** (`appthere-conformance/schemas/` = README only, 0 `.xsd`/`.rng`), zero visual goldens, uncalibrated SSIM threshold, and the `vello_cpu` render path (no `vello` dep in the conformance crate). These are decisions awaiting implementation, not completed work — easy to mistake for "done." See §5.

---

## 2. In-code deferrals — `TODO(topic)` (verified still-open)

All are genuine, mostly upstream-gated (Parley/Blitz/Vello) or deliberately deferred UX polish. The stale/partial ones (S-2, S-3) are in §1.

| Topic | file:line(s) | Defers what |
|---|---|---|
| `3b-3` (partial) | `navigation.rs` (many) | ~~Left/right at page edges + `page_index` recompute after split/merge~~ **fixed 2026-07-05** (plan 4b.1: cross-page Left/Right + `page_locate::recompute_page_index` after every mutation/move). Remaining: double-Enter list-exit heuristic (`clear_para_props`) |
| `loro-bridge` | `loro_bridge/decode.rs` (borders), `table.rs:25` | ~~Non-Rgb colors, comment/bookmark anchors, quote/span attrs~~ **fixed 2026-07-04** (plan Phase 1.4). Remaining: non-Rgb *border* colors (format migration), `Cite` metadata, structural-table CRDT semantics |
| `loro-compaction` | `loki-bench/benches/leak_loro_history.rs`; bridge | ~~Compact the CRDT oplog~~ **fixed 2026-07-04** (plan Phase 1.5): `loro_bridge::compact` + save-point wiring in loki-text; bench asserts the flattened curve. On-device validation pending (BM-14) |
| `omml` | `docx/omml/mod.rs:20` | OMML↔MathML for delimiters, n-ary, matrices, accents |
| `link-click` | `resolve.rs:689`, `items.rs:125`, `para.rs:203`, `scene.rs:519` | Interactive hyperlink hit-testing (only a visual hint today) |
| `shadow` | `para_emit.rs:187`, `para.rs:196`, `scene.rs:93` | True soft text shadow (Vello blur) — hard grey offset copy today |
| `partial-render` | `scene.rs:148`, `editor_pointer.rs:139` | Viewport clipping / direct `node.scroll_offset` |
| `inline-image-flow` | `resolve.rs:706`, `flow_para.rs:213` | Parley inline image boxes (images prepended block-level today) |
| `floating-image` | `resolve.rs:705` | Detect "floating" class for inline images (gap #12) |
| `underline-style` / `strikethrough-style` | `para.rs:164,170` | Double/Dotted/Dash/Wave underline; double strikethrough (all render Single) |
| `super-sub` (partial) | `para.rs:177,870` | Native Parley `BaselineShift` (effect already applied — see S-3) |
| `split-optimise` | `para.rs:409` | Y-range item filter to avoid GPU clipping (Option B; Option A shipped) |
| `tab-default` | `para.rs:648` | Honour `DocumentSettings.default_tab_stop_pt` (hardcoded 36pt) |
| `spell-baseline` | `para.rs:1619` | Tighten squiggle to run underline offset |
| `list-picture-bullet` | `para.rs:1795` | Picture bullets (falls back to `•`) |
| `rotated-cell-editing` | `flow.rs:1676` | Editing data for rotated table cells (read-only today) |
| `pdf-rotate` | `pdf/src/page.rs:83` | Rotation transform in PDF export |
| `odf-master-page` | `odf/reader/styles.rs:200` | ODF master-page transitions |
| `odt-fidelity` | `editor_load.rs:84,88` | Tracked DOCX/ODT import gaps |
| `formatting` | `editor_formatting.rs:106` | Multi-block-selection formatting (clamped to focus paragraph) |
| `undo-dirty` | `editor_state.rs:118` | ~~Saved-vs-undo-stack clean tracking~~ **fixed 2026-07-05** (plan 4b.3): `editing/saved_state.rs` clean checkpoint — undoing back to the save point clears dirty |
| `nested-nav` | `navigation.rs:138,174` | ~~Sibling path inside cell/note body~~ **fixed 2026-07-05** (plan 4b.4): paginated navigation is path-aware; Left/Right cross cell/note siblings and clamp at the container edge |
| `tabs` | `shell.rs:148`, `home.rs:89` | Tab-driven (vs router) navigation; blank-doc |
| `ux` | `text/home.rs:266` (+ presentation/spreadsheet) | Confirm-before-delete dialog (delete is immediate) |
| `browse-templates` / `title-edit` | `text/home.rs:355`, `title_bar.rs:133` | Template browser dialog; inline-editable title |
| `a11y` | `status_bar.rs` (several) | Expand invisible touch targets to `TOUCH_MIN` |
| `icons` | `template_gallery.rs:70`, `document_tab.rs:79`, `title_bar.rs:116` | Real illustration / Tabler icon / SVG app icon (emoji placeholders) |
| `ribbon` / `theme` / `platform` / `font` | various | Ribbon separator variant; **light-theme tokens**; macOS traffic-light region / real OS check; verify bundled UI fonts registered |

---

## 3. External-limitation workarounds — `COMPAT` + dependency patches

### COMPAT clusters (still real unless noted in §1 S-1)

| Cluster / limitation | Reps + count | Still real? |
|---|---|---|
| dioxus-native: `position: fixed` collapses to `absolute` | patches note; `button.rs` (1) | Real |
| dioxus-native: CSS `:hover` → JS `onmouseenter/leave` | `ribbon/button.rs:34` (2) | Real (unconfirmed) |
| dioxus-native: `white-space:nowrap` / `text-overflow:ellipsis` omitted | tab/title/select (~6) | Real (unconfirmed) |
| dioxus-native: SVG via Blitz unconfirmed | `icons.rs` (4) | Real (unconfirmed) |
| dioxus-native: `scrollbar-width/color`, `min-width:0` on flex child | canvas/style (3) | Real (unconfirmed) |
| dioxus-native: winit drag-region / Taffy definite-height | title/shell (4) | Real |
| dioxus: `signal.set()` during render | `editor_state.rs:161` (1) | Real |
| android-mali: Mali-G715 Vulkan device-loss → single-thread init / `use_cpu` / area-AA | `vello_init.rs` (~6) | Real (Mali driver) |
| android-16: `ANativeActivity_onCreate` fires twice | `android.rs:53` (2) | Real (OS) |
| microsoft: Word/OOXML file quirks (bookmark ids, dup styleId, missing Normal, `tblW pct`, `vMerge`) | docx mapper (~7) | Real (permanent) |
| ooxml-dxa / odf: unit + element quirks (twips÷20; ODF column/master-page/self-closing) | reader/mapper (~8) | Real (permanent) |
| loro / loro-schema: Debug-repr serialization (VerticalAlign, tab_stops) | `editor_formatting.rs:207`, bridge | Real (tech debt — see §6) |
| parley-0.6 / vello-0.6 / blitz: no geometric H-metrics; Y-up negation; `push_layer` clip-only transform; `Rgba8Unorm` match | layout/vello (~7) | Real (upstream) |

> ~12 further `COMPAT(dioxus-native)` notes document features CLAUDE.md lists as **confirmed working** (`overflow-x/y:auto`, `flex:1`, `calc(100vh-Npx)`, `box-sizing`, `data-*`, `role`) — these are accurate documentation, not workarounds, and need no action.

### Dependency patches (`docs/patches.md`) — all upstream-gated except one

| Patch | Removal condition | Status |
|---|---|---|
| `dioxus-native-dom` 0.7.9 | upstream implements non-panicking IME/touch event converters | Gated on upstream — not met |
| `blitz-shell` 0.2.3 | upstream native `WindowEvent::Touch` forwarding + `UiEvent` touch variants | Gated — not met. (Tooltip sub-note's "supports `position:absolute`" is **partially staled** — absolute works, fixed doesn't.) |
| `blitz-net` 0.2.1 | upstream ships rustls by default / feature flag | Gated — not met |
| `dioxus-native` 0.7.9 | upstream calls `request_redraw()` after head-element/event processing | Gated — not met |
| `blitz-dom` 0.2.4 | upstream: tabindex focus-on-click, scroll dispatch, absolute node-scroll API, static-canvas anim fix | Gated — not met |
| ~~**`loki-file-access` 0.1.2**~~ | push fixes to the (same-team) `appthere/loki-file-access` repo, publish, repoint | **REMOVED 2026-07-05** (plan Phase 2) — upstreamed as 0.1.3 (`d2b7bc5` on `main`), patch + vendored dir deleted, build scripts repointed via cargo metadata. 5 patches remain, all upstream-gated. |
| ~~`fontique`~~ | — | Already removed 2026-06-21. |

---

## 4. Spec-level deferrals (Spec 01–06)

Verified against code; none silently done-since except the Spec-02 "resolved-as-decision" caveat (§1 note).

| Spec | Deferred item | Verified |
|---|---|---|
| 01 | `clippy::pedantic` lint set + allow-list; AST-level `no_hardcoded_layout_dims` dylint; `cargo udeps` dead-`pub` sweep; `editor_save` typed `SaveError`; Android target build verification; 300-line backlog | STILL-OPEN (deliberate residuals) |
| 02 | ~~`vello_cpu` path, schemas, goldens, calibration, differ, rasterizer, Gelasio, CI wiring~~ **BUILT 2026-07-05** (plan Phase 3): B-1 (`loki-render-cpu`), B-2 (3 ODF goldens + generation script), B-3 (`CALIBRATION.md` → `Tolerance::calibrated()`; the pass quantified fidelity gap #23/kerning, pinned as a canary), B-4 (SSIM+ΔE worst-region differ + heatmaps), B-5 (`PdfRasterizer`), B-6 (ISO 29500/OPC/ODF/MathML3 vendored + real-export validation), B-10 (Gelasio + substitution suite), B-11 (axes live in CI). Remaining: B-8 `Fixture`/`Consumer` traits, B-9 corpus reorg, OOXML manual goldens, Strict XSDs. | LARGELY BUILT |
| 03 | Metadata-panel label stacking <250px (R-13g); responsive doc type-scale (M4); real `Viewport.zoom`; ribbon tab-strip touch height (handed to Spec 04) | STILL-OPEN |
| 04 | **M3 width-driven collapse cascade** (condensed/overflow menu, priority, hysteresis) — unbuilt; **M5 Layout/References/Review tabs + `selected_object` contextual signal** — unbuilt (only 3 non-contextual tabs); M6 touch posture; cursor-into-new-cell after insert | STILL-OPEN (Spec 04 is the least-complete "shipped" spec) |
| 05 | **Page** family (`page_styles` catalog, ADR-0012); **Table** family (`TableProps` conditional/banding regions); character-style **editing form**; per-family non-paragraph `Default` sources; Compact tree breadcrumb (M7) | STILL-OPEN (model-gated) |
| 06 | `vello_cpu` render-cost proxy + parity **execution** (BM-3, blocks on Spec 02); GPU frame-time execution (`device` feature); on-device RSS recalibration + macOS/Windows readers (BM-14); Loro compaction; inactive-tab layout retention (BM-8); per-tile font-byte dedup (BM-9) | STILL-OPEN (device-/upstream-gated) |

---

## 5. Prior-audit open findings (memory / perf / security / fidelity)

Still-open after verification (the DONE-SINCE ones moved to §1).

| Source | Item | Verified |
|---|---|---|
| memory-audit F3 | Drop preserved layout for inactive tabs (`sessions.rs:39` still retains `Arc<PaginatedLayout>`) | STILL-OPEN |
| memory-audit F5 | Share render `FontDataCache` (per-tile `page_paint_source.rs:53` vs shared `DocPageSource`) | STILL-OPEN |
| memory-audit F6 | Compact Loro oplog (`TODO(loro-compaction)`) | ~~STILL-OPEN~~ **FIXED 2026-07-04** (plan Phase 1.5) |
| audit-2026-06 Q-1 | 300-line ceiling backlog — 43→**35**, CI-ratcheted (PARTIAL; `para.rs`/`flow.rs` grew) | PARTIAL |
| audit-2026-06 Q-2 | App-shell duplication (per-app `routes/`, `shell.rs`) | PARTIAL |
| audit-2026-06 Q-3/Q-4 | 301 `let _ =` writer error-swallows (downgraded); ~100 `#[allow]` incl. 32 `dead_code` OOXML | STILL-OPEN (downgraded/P2) |
| audit-2026-06 P-3/P-5/P-6, S-1b/S-2/S-3/S-5 | Glyph-run scans; coarse cache invalidation; cold-path clones; nested-table drop; dim clamp; UTF-16 odd byte; XXE comment | STILL-OPEN (P2/P3, not re-driven) |
| audit-2026-06 T-2/T-3/T-5 tails | ODT export impl; per-case DOCX/XLSX round-trips; hard PPTX cases | STILL-OPEN |
| fidelity gap #12 | External-URL images → grey placeholder (`loki-vello/src/image.rs:34`) | STILL-OPEN |
| fidelity gap #19 | RTL/bidi direction not forwarded (no Parley bidi API) | STILL-OPEN |
| fidelity gaps #23,#25,#26,#27,#29,#30 | ~~kerning~~ (**#23 FIXED 2026-07-05** — `StyleSpan.kerning` + reference-matching off default, found by the Spec 02 calibration pass), orphan/widow, `border_between`, DocxSettings, content controls, language tags | STILL-OPEN (except #23) |
| fidelity-status registry | even/odd blank pages; unequal column widths; column height balancing; drop-cap editor fallback; PDF font subsetting; PDF ICC/CMYK; PDF clip/rotate paint; EPUB drops math/fields/comments; ODT `style:default-style`; macOS symbol-bullet fallback; reflow touch select (selection-delete resolved 2026-07-05); ACID headless raster + ODP/ODG importers + PPTX fixture; Calc/Slides squiggle rendering; personal-dictionary persistence | STILL-OPEN (registry-tracked) |

> **F1–F7** (audit-2026-06-10 app-layer: presentation tab-switch edit loss, no-op delete/copy, dead retier channels, no Save-As) were **not individually re-driven** this pass — they are app-layer and echoed in the MVP-scope doc §6; treat as likely-open pending a focused check.

---

## 6. Known tech debt (code-confirmed)

| Item | Status |
|---|---|
| 300-line-ceiling backlog | **35** baselined files; CI-ratcheted so it can only shrink. `CLAUDE.md`'s worst-offenders sizes are stale (see S-9). |
| `tab_stops` Loro round-trip | ~~STILL-OPEN~~ **FIXED 2026-07-04** (plan Phase 1.1) — structured codec written + read back; `bridge_tab_stops_roundtrip`. |
| paragraph `background_color` Loro round-trip | ~~STILL-OPEN~~ **FIXED 2026-07-04** (plan Phase 1.2) — total `DocumentColor` codec (`loro_bridge/color_codec.rs`); `bridge_para_background_color_roundtrip`. |
| `DocumentMeta`/DublinCore export | **DONE** (writes back to DOCX/ODT) — `CLAUDE.md` row is stale (see S-8). |
| CRDT bridge stubs | ~~debug-log-only~~ **FIXED 2026-07-04** (plan Phase 1.3) — `BulletList`/`OrderedList`/`BlockQuote`/`Div`/`Figure` now have native mappings (`loro_bridge/containers.rs`, `table.rs` pattern: JSON metadata + live nested block lists); tested by `loro_bridge_container_tests.rs`. Legacy pre-opaque stubs still read as `HorizontalRule` (nothing recoverable); `DefinitionList` and inline fields/math stay on the opaque path. |

---

## 7. Out-of-MVP scope (Loki Calc / Loki Slides)

From `docs/mvp-scope-spreadsheet-presentation-2026-06-13.md` — deliberately post-MVP, not defects:

- **Calc:** dead ribbon chrome (tab-select/collapse/zoom are no-ops); row/col virtualization above 500×52; richer formulas (COUNTIF/ROUND/text/comparison, string/bool types); type-to-edit; Shift+Arrow range select. Out of scope: charts, multi-sheet tab UI, frozen panes, cell comments, conditional formatting, find/replace, copy/paste, row/col resize+insert/delete.
- **Slides:** PPTX image/group export; real masters/layouts/theme derivation; ODP import; run-level formatting; shape add/move/resize + Loro undo; faithful per-shape layout (HTML/CSS flow for MVP, no pixel-exact placement); known bugs (in-memory edits lost on tab switch, dead ribbon/zoom handlers).
- **Cross-cutting:** both apps bypass `loki-i18n` for many strings and have **0 tests**.

---

## 8. Recommended actions (from §1) — ✅ applied 2026-07-04

Pure documentation hygiene — no functional change, but stops the docs from misdirecting future work. **All items below were applied in the same pass** (comment rewrites + status-marker corrections; no behaviour changed):

1. Update the four stale **memory/audit/fidelity** statuses: memory Finding 2 → Fixed (S-4); audit-2026-06-10 C1 (S-5) and S1–S4 (S-6) → resolved; fidelity gap #14 → resolved (S-7).
2. Remove/rewrite the stale **in-code** notes: `TODO(editing)` (S-2), `TODO(super-sub)` wording (S-3), and the ~5 `COMPAT(dioxus-native): position:absolute` notes (S-1).
3. Reconcile the two **`CLAUDE.md`** rows: DublinCore export (S-8) and the worst-offenders ceiling sizes (S-9).
4. Reclassify the Spec 02 **"resolved-as-decision"** items (Gelasio, schemas, goldens, SSIM, `vello_cpu`) so they read as *decided, not built*.

Everything else in §2–§7 is a genuine, correctly-documented deferral.

---

## 9. Addendum (2026-07-04, later the same day): F1–F7 re-driven

The §5 closing note flagged the app-layer findings F1–F7 (audit-2026-06-10) as
"not individually re-driven; treat as likely-open pending a focused check."
That focused check has now been done (Phase 0 of
[`deferred-features-plan-2026-07-04.md`](deferred-features-plan-2026-07-04.md));
per-sub-item verdicts against HEAD `20b05a6`:

| # | Original claim | Verdict | Evidence / residual |
|---|---|---|---|
| F1 | Presentation editor is a hardcoded demo; no load/save | **RESOLVED** (core) | Real PPTX import (`editor_load.rs:40-54` → `PptxImport`) and export with Save/Save As (`editor_save.rs`, `editor_inner.rs:87-149`); ODP is a typed `UnsupportedFormat` (deferred by MVP scope, not faked). **Residual resolved 2026-07-05** (plan 4b.6): tab switches now stash/restore the live presentation via `sessions.rs` + `editor_path_sync.rs` (and loki-spreadsheet got the same treatment). |
| F2 | Recents Delete/Copy are silent no-ops (3 apps) | **RESOLVED** | `FileAccessToken::delete()` / `copy_bytes_to()` exist (`patches/loki-file-access/src/token.rs:116,132`) and all three `home.rs` handlers use them; failures surfaced via `pick_error` + `errors.ftl` keys. The `TODO(ux)` confirm-dialog landed 2026-07-05 (plan 4c.1, `AtConfirmDialog`). |
| F3 | Edits lost on tab switch; dirty flag never set | **LARGELY RESOLVED** | Per-tab retention via `loki-text/src/sessions.rs` `DocSession` (stash/restore in `editor_path_sync.rs:42-154`); dirty tracks a generation baseline (`editor_inner.rs:465-476`), cleared on save. **Residual (F3c) resolved 2026-07-05** (plan 4b.6): closing a dirty tab raises an `AtConfirmDialog` confirmation in all three apps' shells. |
| F4 | Untitled documents cannot be saved; no Ctrl+S | **RESOLVED** | Save As via `pick_file_to_save` (`editor_inner.rs:484-535`); Ctrl/Cmd+S bound (`editor_keydown.rs:60-67`) routing untitled → Save As. |
| F5 | Settle/retier pipeline wired to dead channels | **RESOLVED (by removal)** | The pipeline was deleted, not fixed: virtualization now bounds memory by mounting only viewport-window pages (`virtualize.rs` `visible_window`, `document_view.rs:290`). The 06-10 audit's §5 claim of "downsample by viewport distance" was wrong about the mechanism — corrected there. **Residual:** `DocumentViewProps::eq` still hardwired `false` (`document_view.rs:143-147`) — now a benign over-render, capped by `PageTile`'s own `PartialEq`. |
| F6 | Medium grab-bag | **PARTIAL** | RESOLVED: F6b hit-testing geometry (live `client_width`/`scroll_offset` via `scroll_metrics`), F6e spreadsheet (visible save errors, SUM/COUNT/AVERAGE/MIN/MAX/IF engine, dynamic grid to 500×52), F6g onscroll panic path (`convert_scroll_data` implemented; unimplemented converters have no registered handlers), F6h i18n variant-locale fallback (parsed-langid comparison + regression test). PARTIAL: F6d dead UI — loki-text ribbon tabs/collapse/template cards fixed; **zoom controls dead in all 3 apps; spreadsheet ribbon tab-select/collapse dead**. RESOLVED 2026-07-05: F6c-selection — typing replaces the active selection and Backspace/Delete remove it, including multi-block ranges (`loki_doc_model::delete_selection_at` composes `merge_block_at` + `delete_text_at` with pre-validation, so cross-container or table-spanning ranges are rejected untouched; wired via `editor_keydown_text.rs`; tested by `loro_selection_delete_tests.rs` + editor unit tests). STILL-OPEN: F6a hooks in conditionals/loops (`recent_files.rs:45,96,219`), F6c-clipboard (copy/cut/paste — partially gated on the unimplemented dioxus-native-dom clipboard converter), F6f synchronous save/load on UI thread. |
| F7 | Low grab-bag | **PARTIAL** | RESOLVED: F7d safe-area insets (RwLock + reactive version signal + resize sensor). PARTIAL: F7c — loki-text page number now live; word count still empty everywhere. STILL-OPEN: F7a `AtHomeTab` responsive layout (`viewport_width` fixed 375.0, never adopted `use_breakpoint()`), F7b index-based list keys + `active_slide_idx` not adjusted on delete-before-active, F7e debug leftovers in vendored patches, F7f `buttons ^= Main` XOR on touch end/cancel (`patches/blitz-shell/src/window.rs:1133`). |

**Two stale in-code comments found during verification were fixed in the same
pass:** the `TODO(undo-dirty)` parenthetical ("Save not implemented" — Save now
exists; remaining work is the undo-stack clean checkpoint) and the
`editing/hit_test.rs` doc-comment claiming `scroll_offset = 0.0`.

The confirmed-open items fold into the plan as follows: F3c + F1-residual
(close/switch protection for dirty work) join Phase 4b; F6a/F6c/F6d/F6f and
F7a/F7b/F7c join the Phase 4b/4c backlog; F7e/F7f are patch-tree fixes queued
with the next patch re-vendor (Watch list); the F5 `PartialEq` residual joins
Phase 6 (perf polish).
