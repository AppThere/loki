<!--
SPDX-License-Identifier: Apache-2.0
-->

# Deferred-Features Remediation Plan (2026-07-04)

| | |
|---|---|
| **Status** | Plan — no code changes in this document's pass. |
| **Source** | [`docs/deferred-features-audit-2026-07-04.md`](deferred-features-audit-2026-07-04.md) — every task below cites its audit section. |
| **Already done** | The audit's §8 documentation-hygiene actions (S-1…S-9 stale-doc fixes) were applied on 2026-07-04 and are **not** in this plan. Verified: no stale `TODO(editing)` / `position: absolute` COMPAT notes remain in code, and the `CLAUDE.md` rows are reconciled. |
| **What this plan covers** | The genuine, correctly-documented deferrals in audit §2–§7, sorted into: work we can and should do (Phases 0–7), upstream-gated items we can only watch (§ Watch list), and deliberate post-MVP scope we will not schedule (§ Out of scope). |

## How to read this plan

- **Priority**: P0 = correctness/data-integrity or unblocks other phases; P1 = committed spec work left unbuilt; P2 = quality/perf/fidelity backlog; P3 = polish.
- **Effort**: S ≤ 1 day, M ≤ 1 week, L > 1 week (single engineer, familiar with the crate).
- Phases are ordered by priority but are largely **independent workstreams** — they can proceed in parallel except where a dependency is called out.
- Every task keeps the repo conventions: root-cause fixes only, 300-line ceiling (ratcheted), `cargo fmt` + `clippy -D warnings`, and a `docs/fidelity-status.md` update for any layout/rendering/import-export change.

---

## Phase 0 — Verification pass: re-drive the unverified findings (P0, S)

The audit explicitly did **not** re-drive the app-layer findings F1–F7 from
`audit-2026-06-10` (presentation tab-switch edit loss, no-op delete/copy, dead
retier channels, no Save-As) — they are "likely-open pending a focused check"
(audit §5, closing note). Planning fixes against unverified findings risks the
exact stale-doc failure mode the audit was written to catch.

| Task | Detail | Exit criterion |
|---|---|---|
| 0.1 | ✅ **Done 2026-07-04** — F1–F7 re-driven; verdicts in the audit's new §9 addendum. Headline: F1/F2/F4 resolved, F3 largely resolved, F5 resolved-by-removal, F6/F7 partial. | Each of F1–F7 has a verified status with `file:line` evidence. |
| 0.2 | ✅ **Done 2026-07-04** — confirmed-open subset folded into Phases 4/6 and the Watch list (see 4b.6/4b.7, 4c.5, 6.7 below). | Phase 4 backlog updated. |

---

## Phase 1 — CRDT round-trip integrity (P0, M)

Data that survives import but is silently degraded by the Loro bridge is the
closest thing to data loss in the suite. These are all code-confirmed in audit
§6 and §2 (`loro-bridge` topic).

| Task | Source | Detail | Effort |
|---|---|---|---|
| 1.1 | §6 | ✅ **Done 2026-07-04** — structured tab-stop codec (`loro_bridge/decode.rs`) + reader; `bridge_tab_stops_roundtrip`. | S–M |
| 1.2 | §6 | ✅ **Done 2026-07-04** — total `DocumentColor` codec (`loro_bridge/color_codec.rs`, Rgb/Cmyk/Theme/Transparent) + reader; `bridge_para_background_color_roundtrip`. | S |
| 1.3 | §6 | ✅ **Done 2026-07-04** — native mappings for `BulletList`/`OrderedList`/`BlockQuote`/`Div`/`Figure` (`loro_bridge/containers.rs`, table.rs pattern); `loro_bridge_container_tests.rs`. `DefinitionList` + inline fields/math stay opaque. | M |
| 1.4 | §2 | ✅ **Done 2026-07-04** — char colors use the total `DocumentColor` codec (Theme/Cmyk/Transparent survive, mark + block-map paths); comment/bookmark anchors preserved as `OBJECT_REPLACEMENT_CHAR` snapshot marks; `Quoted` quote-type and `Span` attrs carried as range marks with recursive child writes; `loro_bridge_inline_tail_tests.rs`. Remaining TODO(loro-bridge): non-Rgb *border* colors (colon-format migration), `Cite` metadata, structural-table CRDT semantics. | M |
| 1.5 | §2, §5 | ✅ **Done 2026-07-04** — `loro_bridge::compact` (`compact_in_place` / `compact_history`) wired at the save point in loki-text (`editor_compact.rs`; ribbon Save unified into the Ctrl+S handler). Bench acceptance passed: 5 000 keystrokes = 19 208 ops uncompacted → 1 op compacted, asserted in `leak_loro_history`. On-device long-session validation still pending (BM-14). | M |

**Exit criteria**: `metadata_round_trip.rs`-style tests pass for 1.1–1.3; the
`leak_loro_history` bench shows bounded history; the two §6 tech-debt rows and
the bridge-stubs row in `CLAUDE.md` are updated to DONE.

---

## Phase 2 — The one actionable dependency patch: `loki-file-access` (P0, S)

Audit §3: of the 6 `[patch.crates-io]` entries, five are gated on upstream
Dioxus/Blitz releases (watch list), but **`loki-file-access` 0.1.2 is
same-team-owned** (`appthere/loki-file-access`) and its removal condition is
entirely in our hands.

| Task | Detail |
|---|---|
| 2.1 | ✅ **Done 2026-07-05** — full patch content (Android NativeActivity fixes, Java shims + dexing `build.rs`, insets/IME bridges, `token.delete()`/`copy_bytes_to()`) upstreamed to `appthere/loki-file-access` as **0.1.3**, commit `d2b7bc5` fast-forwarded to `main` (fix branch `claude/upstream-android-nativeactivity-fixes` also pushed). 43 tests + clippy clean upstream. |
| 2.2 | ✅ **Done 2026-07-05** — `[patch]` entry and `patches/loki-file-access/` removed; the `branch = "main"` git dependency resolves to 0.1.3 directly. The three Android build scripts now resolve the Java-shim directory from `cargo metadata` instead of the deleted patch path. Registry publication remains optional (the workspace dep is git, not registry) — noted in `docs/patches.md` "Removed patches". |

**Exit criterion**: ✅ met — `cargo check --workspace` with no `loki-file-access`
patch and zero `Patch ... was not used` warnings.

---

## Phase 3 — Conformance foundation: build the Spec 02 "resolved-as-decision" items (P1, L)

Audit §1 note + §4: several Spec 02 items carry a "✅ Resolved" *decision* but
were never built — verified again 2026-07-04 (no Gelasio face in
`loki-fonts/fonts/`, no vendored schemas, zero goldens). This phase turns
decisions into artifacts, in dependency order. It also **unblocks Spec 06
BM-3** (render-cost proxy) and the ACID headless-raster registry item.

| Task | Spec item | Detail | Effort |
|---|---|---|---|
| 3.1 | B-10 | ✅ **Done 2026-07-05** — Gelasio ×4 faces bundled (OFL, full coverage reconstructed from fontsource subsets), Georgia→Gelasio mapping, dedicated substitution suite (`loki-layout/tests/font_substitution_suite.rs`). | S |
| 3.2 | B-6 | ✅ **Done 2026-07-05** — ISO 29500-4:2016 Transitional + mce, ECMA-376 Part-2 OPC, OASIS ODF 1.3 RNG + MathML3 vendored with PROVENANCE (sha256); real DOCX/ODT exports schema-validated incl. malformed-part negative tests. Tails: Strict XSDs, Dublin Core imports for core.xml (documented in `schemas/README.md`). | M |
| 3.3 | B-1 | ✅ **Done 2026-07-05** — new `loki-render-cpu` crate: `vello_cpu` (=0.0.9) rasterizes the same `PositionedItem` stream the GPU path paints, headless, byte-deterministic (M5 acceptance smoke tests). TODO(conformance-render): decode image items (grey placeholder today). | M |
| 3.4 | B-5 | ✅ **Done 2026-07-05** — `appthere_conformance::raster::PdfRasterizer` (pdftoppm pinned flags @ `CONFORMANCE_DPI` 144, version captured, byte-determinism tested). | S |
| 3.5 | B-2, B-3, B-4 | ✅ **Done 2026-07-05** — SSIM+CIEDE2000 worst-region differ with heatmaps (`golden/diff.rs`, Sharma reference pairs verified); 3 ODF goldens committed with GENERATION metadata (`scripts/generate-odf-goldens.sh`); calibration record `goldens/CALIBRATION.md` → `Tolerance::calibrated()` {0.60, 10.0}. **The calibration pass found and quantified fidelity gap #23 (kerning)** — pinned as the `para-carlito` expected-failure canary. OOXML goldens remain the manual Windows/Word COM procedure (§7.2). | M |
| 3.6 | B-11 | ✅ **Done 2026-07-05** — all three axes run as cargo tests in the existing `build-and-test` job (xmllint + pdftoppm installed); schema + round-trip are hard gates, visual is advisory-by-construction (known divergence pinned; hardens when kerning lands + recalibration). | S |
| 3.7 | B-8, B-9 | **Deferred** — shared `Fixture`/`Consumer` trait extraction from `loki-acid` and the 141-case corpus reorg remain open (the new fixtures/goldens live under `appthere-conformance/{fixtures,goldens}` as the seed of that layout). ODP/ODG/PPTX importers stay gated on the unbuilt ACID PPTX generator (§5.1). | M |

**Exit criterion**: ✅ met for the built scope — CI runs schema, round-trip,
and visual-golden passes on every PR; Spec 02 rows B-1…B-6, B-10, B-11 are
*built*, B-7 was already largely done, B-8/B-9 remain the tracked tail.

---

## Phase 4 — Editor completion: Spec 04/05 gaps + verified UX TODOs (P1, L)

The audit calls Spec 04 "the least-complete 'shipped' spec" (§4). This phase
groups the user-visible editor work: spec milestones first, then the §2 UX
TODOs (merged with whatever Phase 0 confirms from F1–F7).

### 4a. Spec milestones (model/architecture-gated first)

| Task | Source | Detail | Effort |
|---|---|---|---|
| 4a.1 | Spec 04 M3 | Width-driven ribbon collapse cascade: condensed variant, overflow menu, per-group priority, hysteresis. **Collapse engine ✅ Started 2026-07-06** — the pure, width-driven cascade core lives in `appthere-ui/src/responsive/ribbon_collapse.rs` (`resolve_cascade(metrics, available_px, prev_level) -> RibbonCascade`): each group declares a `GroupMetrics { priority, full_px, condensed_px }`, and the engine degrades gracefully by **condensing all groups (lowest priority first) before overflowing any** into the "More" menu, falling back to horizontal scroll only when even the fully-overflowed strip can't fit (§7 steps 1–4). Per-group result is `GroupCollapse::{Full,Condensed,Overflow}`. **Hysteretic** like Spec 03's `page_fit`: it collapses a step the instant the strip overflows but re-expands only when the looser layout clears the width by `RIBBON_COLLAPSE_HYSTERESIS_PX` (32 px), so a window dragged across a fit threshold doesn't thrash; resolution is idempotent at a fixed width (the resolved `level` feeds back in as `prev_level`). New tokens `RIBBON_OVERFLOW_BUTTON_PX` (44) + `RIBBON_COLLAPSE_HYSTERESIS_PX`. 10 unit tests (`ribbon_collapse_tests.rs`) cover full-fit, priority-ordered condense/overflow, the scroll floor, hysteresis dead-band, idempotence, tie-breaking, and the empty ribbon. **Reactive binding + condensed representation ✅ 2026-07-06** — (a) `use_ribbon_cascade(metrics) -> RibbonCascade` binds the engine to the live `AtResponsiveContext` viewport width, holding the resolved `level` in a hook-local signal for hysteresis across resizes; it is **resilient** like `use_breakpoint` (no responsive context ⇒ unbounded width ⇒ every group stays Full, so Presentation/Spreadsheet get a sane full-chrome ribbon). (b) The condensed group representation (§7 step 2) is a pure, tested decision — `group_layout(collapse, has_label) -> GroupLayout { rendered, pad_px, gap_px, show_label }` — that `AtRibbonGroup` now applies via a new defaulted `collapse: GroupCollapse` prop: **Full** keeps the label + roomy padding, **Condensed** drops the label and tightens padding/gap (never the buttons, so 44 px touch targets survive), **Overflow** renders nothing in the strip (the "More" menu hosts it). The prop defaults to Full, so all existing call sites are unchanged. +3 `group_layout` tests (13 total). **Collapse-aware container + overflow menu + first migration ✅ 2026-07-06** — a new shared `AtRibbonGroups` component (`components/ribbon/groups.rs`) is the framework piece that owns the cascade: a tab hands it a `Vec<RibbonGroupSpec { metrics, label, aria_label, content }>`, it runs `use_ribbon_cascade` **once** for the strip, renders each group at its resolved state, and moves overflowed groups into a trailing **"More" menu** (an upward `position: absolute` dropdown — confirmed working in Blitz — rendering the overflowed groups in Full form). Group widths are **declared, not Blitz-measured** (per-element measurement is unreliable): the pure `estimate_group_metrics(priority, buttons, has_label)` derives full/condensed widths from the touch-button count (+2 tests). The **Layout tab** is migrated as the first real consumer — its four groups (Orientation/Margins/Size/Columns, priority descending so Columns overflows first) now flow through `AtRibbonGroups`, driven live off the editor's measured viewport width. New `LUCIDE_MORE_HORIZONTAL` icon + `ribbon-overflow-aria` string. **All tabs migrated + R-13e ✅ 2026-07-06** — the **Write**, **Insert**, **Publish**, and **Table** tabs now build `RibbonGroupSpec` lists through `AtRibbonGroups` too, so every tab collapses/overflows by priority (the group-helper fns in `editor_ribbon_format`/`editor_ribbon_color` return `RibbonGroupSpec` with a threaded priority; each tab declares a descending-importance scheme — core editing controls stay full longest, wide colour-swatch groups overflow first). Publish's ribbon content split to `editor_ribbon_publish.rs` (dropping the baselined `editor_publish.rs` 315 → 236, off the ceiling backlog) and the Table tab's `delete_current_table` to `editor_ribbon_table_delete.rs`. **R-13e ✅** — `AtRibbonGroup` exposes its resolved `GroupCollapse` to descendants as a *signal* context, and `AtRibbonSelect` reads it to shrink (`RIBBON_SELECT_WIDTH_PX` → `RIBBON_SELECT_WIDTH_CONDENSED_PX`) when its group is condensed; a signal, not a plain value, so prop-memoised selects still re-size reactively on resize. The overflow menu also auto-closes when a widen removes the overflow. **Remaining tail:** true outside-click-to-dismiss for the More menu — it needs the menu hosted in a shared **window-level overlay** (like the editor-root overlay the spell panel uses) so a full-viewport backdrop can span the viewport; a backdrop rendered in-place can't (`position: fixed` collapses to `absolute` in stylo_taffy). Toggle-close + auto-close-on-widen ship today; `TODO(ribbon)` marks the overlay-host follow-up. **M3 is otherwise complete.** | L |
| 4a.2 | Spec 04 M5 | Layout/References/Review ribbon tabs + ~~`selected_object` contextual-tab signal~~ (✅ **Done 2026-07-06** — new `editing/selected_object.rs`: pure `selected_object(&CursorState) -> SelectedObject` (Table when the focus path descends through a cell). `editor_ribbon_table::use_ribbon_tabs` derives it via `use_memo`, appends an amber **Table** contextual tab while the caret is in a table, and resets the active tab to Write when the caret leaves (so no orphaned selection). The tab's **Delete Table** action is backed by a new `delete_block` model mutation (`loro_mutation/block_edit.rs`, split from `block.rs` to hold the ceiling), disabled when the table is the document's only block; the caret re-homes to the neighbouring block. New `LUCIDE_TRASH_2` icon. Tests: `selected_object` (5), `ribbon_tabs` (3), `delete_block` (3). **Structural row/column ops ✅ Done 2026-07-06** — the Table tab's "Rows & Columns" group inserts/deletes rows and columns via new `loro_mutation/table_ops.rs` mutations (`insert_table_row`/`delete_table_row`/`insert_table_column`/`delete_table_column` + `table_grid_dims`): they rewrite the serde skeleton **and** minimally patch the flat `KEY_TABLE_CELLS` list (surviving cells keep their live CRDT text), scoped to simple grids (no spans/head/foot — else typed `UnsupportedTableStructure`). The caret re-homes to its shifted cell (pure `caret_flat_after` in `editor_ribbon_table_ops.rs`); delete buttons disable at 1 row/col; 4 app-custom table-op glyphs. Tests: 12 round-trip (`table_structural_ops.rs`) + 4 caret-math. **Insert above/below + left/right ✅ Done 2026-07-06** — the insert ops are now split into four caret-relative variants (`TableOp::{InsertRowAbove,InsertRowBelow,InsertColumnLeft,InsertColumnRight}`): above/left insert at the caret's own row/col index, below/right at index+1, and `caret_flat_after` re-homes the caret accordingly (above shifts it down a row, left shifts it one column right). 2 app-custom glyphs (`AT_TABLE_ROW_INSERT_ABOVE`, `AT_TABLE_COL_INSERT_LEFT`); 2 new caret-math tests. **Paragraph alignment ✅ Done 2026-07-06** — the Write tab gains an Alignment group (left/centre/right/justify) driven by new path-aware `set_block_alignment_at`/`get_block_alignment_at` (`loro_mutation/align.rs`), so alignment works in table cells and note bodies too. `align.rs` handles every paragraph shape: a plain `para` is upgraded to `styled_para` so props survive the read path, `styled_para` uses `para_props`, and a `heading` uses its OOXML `jc` attr. The 6 inline-format buttons + the new alignment buttons were extracted to `editor_ribbon_format.rs` (dropping `editor_ribbon.rs` 300 → 225). 5 alignment tests (`block_alignment.rs`). **Font-size grow/shrink ✅ Done 2026-07-06** — a Write-tab Font group steps the selection's `MARK_FONT_SIZE_PT` up/down a fixed size ladder (`editor_font_size.rs`, path-aware via `mark_text_at`/`resolve_format_ranges`, so it works in cells and across multi-paragraph selections); pure ladder + end-to-end mark tests (5). 2 app-custom "A±" glyphs. **Text colour ✅ Done 2026-07-06** — a Write-tab Font-colour group with an Automatic (clear) swatch + 6 preset colours applies `MARK_COLOR` (the `#RRGGBB` hex is the codec's own RGB form, so no extra encoding) across the selection, path-aware (`editor_text_color.rs`); coloured-square swatches render inside `AtRibbonIconButton`. 3 tests incl. round-trip into `CharProps.color`. **Highlight colour ✅ Done 2026-07-06** — a Write-tab Highlight group (None clear-swatch + 5 preset colours: Yellow/Green/Cyan/Magenta/Red) writes `MARK_HIGHLIGHT_COLOR` as a `HighlightColor` variant name (or `Null` to clear) across the selection, path-aware (`editor_highlight_color.rs`); the swatch fills are the RGB each variant renders as (`resolve::map_highlight_color`). The duplicated font-colour/highlight swatch UI was unified into a generic `swatch_group(palette, apply_fn)` in `editor_ribbon_color.rs`. 3 tests incl. round-trip into `CharProps.highlight_color`. **Layout tab ✅ Started 2026-07-06** — a new non-contextual **Layout** tab (Write/Insert/**Layout**/Publish; contextual Table now at index 4) with a page-**orientation** toggle backed by new `set_document_orientation`/`document_is_landscape` model mutations (`loro_mutation/page.rs`): the layout engine reads the effective `page_size` directly, so the toggle swaps width↔height on every section + sets the orientation flag, and `apply_mutation_and_relayout` re-flows at the new size (verified the pipeline updates `page_width_px`). 4 round-trip tests (`page_orientation.rs`) + updated ribbon-tab tests; 2 app-custom page-rect glyphs. **Margin presets ✅ Done 2026-07-06** — the Layout tab's Margins group applies Normal/Narrow/Wide via new `set_document_margins`/`document_margins` (`page.rs`; sets top/bottom/left/right on every section, leaves header/footer/gutter); the active preset highlights via the pure `margin_matches` (½-pt tolerance). 3 model tests (`page_margins.rs`) + 5 preset-match tests; 3 app-custom page-inset glyphs (tooltip-disambiguated). **Page size ✅ Done 2026-07-06** — the Layout tab's Size group applies A4/US-Letter via new `set_document_page_size`/`document_page_size` (`page.rs`), **preserving each section's orientation** (choosing A4 while landscape gives A4 landscape); the active size highlights via the orientation-independent `page_size_matches`. 3 model tests (`page_size.rs`, incl. orientation preservation) + 4 preset-match tests. **Columns ✅ Done 2026-07-06** — the Layout tab's Columns group applies one/two/three via new `set_document_columns`/`document_column_count` (`page.rs`; count clamped ≥1, a new columns map gets a default 0.5in gap + no separator, an existing one keeps its gap/separator when the count changes); 5 model tests (`page_columns.rs`). The Layout tab is now Orientation + Margins + Size + Columns. **Remaining:** the References/Review tabs (need TOC / track-changes mutations). | L |
| 4a.3 | Spec 05 | **Page** style family (`page_styles` catalog per ADR-0012) and **Table** family (`TableProps` conditional/banding regions); character-style editing form; per-family non-paragraph `Default` sources; Compact-tree breadcrumb (M7). **Character-family `Default` source ✅ 2026-07-06** — the first of the per-family non-paragraph `Default` sources (ADR-0012 Decision 1). New `StyleCatalog::default_character_style` (serde-default, so it round-trips through the Loro bridge and is back-compatible); `resolve_char_chain` now falls through to it (`first_in_char_chain`, cycle/depth-guarded) so a standalone character style resolves the document's `docDefaults` run defaults as `Provenance::Default` instead of `FormatDefault` — the char inspector was previously **blind** to docDefaults. The OOXML importer synthesises a `__DocDefaultChar` character style from `w:rPrDefault` and points the default at it; the character browser hides `__`-prefixed synthetic styles, and both the DOCX and ODT writers skip them (they belong in `docDefaults`/`default-style`, not as named `w:style`/`style:style` — also fixes a latent `__DocDefault` paragraph leak). 4 model tests + 3 mapper tests; full OOXML/ODF/round-trip suites green. **Character-style editing form ✅ 2026-07-06** — the character family is now editable, not just inspectable (Spec 05 M6). Selecting a character style seeds an editable `StyleDraft` (`char_style_to_draft`) that a new `char_form.rs` binds — reusing the paragraph form's shared inputs (`field_row`/`iu_buttons`/`font_picker`/`weight_selector`, all of which already bind a `Signal<Option<StyleDraft>>`) for name/based-on/font-family/weight/size/italic/underline. Apply commits a `CharacterStyle` to the catalog through Loro (`commit_char_style_to_loro`, persisted via the existing `write_document_styles` bridge, undoable) and relays out, **cycle-guarded** by new model helpers `char_ancestors`/`char_reparent_cycles` (the character analogue of the paragraph re-parent guard). The editable form renders alongside the read-only provenance inspector (inspector shows *where* inherited values come from, form edits the locals — the Spec 05 §6 inspector+edit pairing). 1 model test (`character_reparent_cycle_is_detected`); `editor_inner` held at its 803 baseline. **Remaining:** table-/list-family `Default` sources (same pattern, ODF `style:default-style` import symmetry); **Page** style family (`page_styles` catalog + `PageLayout`/`sectPr` import + DOCX section-export inverse + the flat page panel, ADR-0012 Decision 2); **Table** `TableProps` conditional/banding editing; and the **Compact-tree breadcrumb** (M7). | L |
| 4a.4 | Spec 03 | ~~Metadata-panel label stacking <250 px (R-13g)~~ (✅ **Done 2026-07-06** — `FieldRow` is now a `#[component]` reading `use_viewport()` per ADR-0013; below `METADATA_LABEL_STACK_PX` (250 px) the label stacks above its input via a `flex-direction: column` switch so the input keeps a usable width; pure `stack_labels` helper + 3 tests in `editor_metadata_panel_tests.rs`); responsive doc type-scale (M4); ~~real `Viewport.zoom`~~ (✅ **Done 2026-07-06** — the status-bar zoom now feeds the shared responsive `Viewport::zoom` (`editor_responsive.rs`: pure `zoom_fraction`/`desired_view_mode` helpers wired into effects 2 & 3), so zooming a page past the point it fits flips the page-fit renderer to reflow instead of forcing horizontal scroll; 5 tests in `editor_responsive_tests.rs`). **Remaining:** responsive doc type-scale (M4). | M |
| 4a.5 | Spec 04 M6 | Touch posture + ~~cursor-into-new-cell after insert~~ (✅ **Done 2026-07-06** — `insert_table_after_cursor` now returns the first-cell caret (`first_cell_caret` → flat cell 0 / block 0), and the Insert-tab `run_insert` collapses the cursor there after relayout via `set_collapsed_cursor` (page re-derived per 4b.1); footnote leaves the caret at the anchor, matching Word. `InsertResult` enum threads the optional caret target; the async image flow was extracted to `editor_ribbon_insert_image.rs` to hold the ceiling. 3 tests in `editor_insert_tests.rs`). **Remaining:** touch posture. | M |

### 4b. Editing-core TODOs (§2)

| Task | Topic | Detail | Effort |
|---|---|---|---|
| 4b.1 | `3b-3` | ✅ **Done 2026-07-05** — (1) Left/Right cross page boundaries (the prev/next-entry searches walk the whole layout; entering a table from above/below lands in its first/last cell). (2) New `editing/page_locate.rs`: `recompute_page_index` re-derives a position's page from the layout, picking the page whose content band shows the byte's line for page-spanning paragraphs; wired into every mutation caret placement (`set_collapsed_cursor` — split, merge, typing, selection removal) and after every navigation move, replacing the stale-page TODOs. 4 page-locate tests (incl. a split-fragment band test) + 4 cross-page nav tests; `navigation.rs` split (`navigation_find.rs`) dropped it from the ceiling baseline (34 → 33). ~~**Remaining `3b-3` tail:** the double-Enter list-exit heuristic (`clear_para_props`).~~ ✅ **Done 2026-07-06** — pressing Enter on an *empty, top-level list item* now removes its list props (`list_id`/`list_level`) instead of inserting another bullet (Word / LibreOffice behaviour): new model mutations `get_block_list_id` / `clear_block_list` (`loro_mutation/style.rs`), a pure `is_empty_list_item_exit` predicate + wiring in `editor_keydown_enter.rs` (caret held in the now-plain paragraph, page re-derived), 4 model tests + 5 predicate tests. Nested list items (in a table cell / note) keep the split — the list block API is top-level only. | M |
| 4b.2 | `formatting` | ✅ **Done 2026-07-05** — the six toggles (bold/italic/underline/strikethrough/super/subscript) apply across every paragraph of a same-container multi-block selection: new `editor_format_range.rs` `resolve_format_ranges` yields per-paragraph `(BlockPath, start, end)` ranges (first-paragraph tail, full middles, last-paragraph head; text-less blocks like tables inside the range are skipped, their cells untouched); the state at the selection start decides apply-vs-clear and the whole selection is made uniform (Word behaviour). Cross-container selections keep the clamp-to-focus rule. 8 tests incl. an end-to-end double-toggle. | M |
| 4b.3 | `undo-dirty` | ✅ **Done 2026-07-05** — undo-stack clean checkpoint: `editing/saved_state.rs` `SavedStateHandle` mirrors the undo depth via the loro `on_push`/`on_pop` hooks (fresh edits arrive with `Some(event)`, undo/redo replays with `None`), save records the clean depth (+ `record_new_checkpoint()`), and the dirty indicator clears when undo/redo returns the stack to it. Editing below the save point marks it permanently unreachable (classic clean-index semantics). Tracker rides with the `UndoManager` through tab-switch stash and the post-save compaction swap. 6 integration tests against a real loro `UndoManager` (`saved_state_tests.rs`). Save As also moved to `editor_save_callbacks.rs`, ratcheting `editor_inner.rs` 870 → 833. **Ribbon Save disables when clean ✅ 2026-07-05:** the dirty state is now a reactive `is_dirty` signal (set by the dirty-tracking effect) threaded into `write_tab_content`; the Save button's `is_disabled = !is_dirty()` (untitled reads as dirty, so Save→Save-As stays enabled). **Remaining tail:** Spec 01's typed `SaveError` residual. | M |
| 4b.4 | `nested-nav` | ✅ **Done 2026-07-05** — paginated navigation is path-aware end-to-end: `find_para_data` matches `(block_index, path)` (index alone returned the first cell's entry for every table paragraph), the `get_text` closures take a `BlockPath` (grapheme moves inside cells previously read the root block's empty text), and Left/Right at a nested paragraph's edges cross to the sibling within the same cell / note body, clamping at the container's first/last block. Inline tests extracted to `navigation_tests.rs` (`#[path]` idiom, file ratcheted 593 → 367) + 6 nested-nav regression tests. Reflow navigation stays top-level-only (tables have no reflow editing data). | S |
| 4b.5 | `rotated-cell-editing` | Editing data for rotated table cells (`flow.rs:1676`) — read-only today. **Note:** `flow.rs` is a top ceiling offender; split it (Phase 7) before or with this change. | M |
| 4b.6 | F3c + F1 residual (audit §9) | **Dirty-close protection ✅ Done 2026-07-05:** closing a dirty tab now raises a confirmation dialog in **all three apps** (new `appthere_ui::AtConfirmDialog` overlay primitive — absolute backdrop + centred card, backdrop-cancel, 44 px touch targets; shells gain `pending_close` state + an extracted `close_tab` helper). **Tab-switch retention ✅ Done 2026-07-05 (F1 residual):** loki-presentation *and* loki-spreadsheet now stash the live editing state (presentation: doc + slide index + dirty; spreadsheet: CRDT + undo manager + grid snapshot + selection) into an app-level session map on tab switch / Editor→Home unmount and restore it on return — mirroring loki-text's `sessions.rs`; closing a tab drops the session. The presentation load path also gained the `(path, result)` stale-value guard the other apps already had. | S–M |
| 4b.7 | F6c + F6f (audit §9) | **Selection editing ✅ Done 2026-07-05:** typing replaces the active selection, Backspace/Delete remove it, incl. multi-block ranges — `loki_doc_model::delete_selection_at` (merge-then-delete composition, whole range pre-validated so cross-container / table-spanning selections are rejected untouched), editor wiring in `editor_keydown_text.rs` (replace-typing is one undo entry); tests: `loro_selection_delete_tests.rs` (10) + editor unit tests (7). **Remaining:** clipboard copy/cut/paste (partially gated on the unimplemented dioxus-native-dom clipboard converter), and moving save/load I/O off the UI thread (`editor_ribbon.rs:93`, `editor_load.rs:56-101`). | M |

### 4c. Shell/UX polish TODOs (§2) — batchable

| Task | Topics | Detail | Effort |
|---|---|---|---|
| 4c.1 | `ux` | ✅ **Done 2026-07-05** — the recents Delete menu action now only *requests* deletion; `AtConfirmDialog` performs it on confirm (all three apps' Home; `home.rs` split `home_util.rs` out to stay under the ceiling). | S |
| 4c.2 | `a11y` | ✅ **Done 2026-07-05 (bounded by bar height)** — the status bar's three interactive controls (notice chip, view-mode toggle, zoom badge) are now transparent hit areas ≥ `TOUCH_MIN` (44 px) wide × the bar's full height, with the visual chip nested inside. **Honest constraint:** the 24 px `STATUS_BAR_HEIGHT` caps the vertical target at WCAG 2.5.8 AA's 24 px minimum, below the suite's 44 px convention — meeting it fully requires a taller bar on touch platforms (design decision, deferred; documented on `AtStatusBar`). | S |
| 4c.3 | `title-edit`, `browse-templates`, `tabs` | Inline-editable title; template-browser dialog; tab-driven navigation + blank-doc. | M |
| 4c.4 | `icons`, `ribbon`, `theme`, `platform`, `font` | Real Tabler/SVG icons over emoji; ribbon separator variant; **light-theme tokens** (currently Dark-only); macOS traffic-light region / real OS check; verify bundled UI fonts registered. | M |
| 4c.5 | F6a/F6d/F7a/F7b/F7c (audit §9) | ✅ **Done 2026-07-05** (spreadsheet grid-zoom is the sole tail). ✅ F6a — recent-file rows and template cards are now child `#[component]`s (`recent_row.rs`, `RecentRow`/`OpenFileButton`/`TemplateCard`/`BrowseCard`) so their hover `use_signal`s no longer live inside `for`/`if` bodies (hook count is now prop-independent; ADR-0013). ✅ F6d ribbon — loki-spreadsheet's ribbon lists only the implemented Home tab (the dead Insert/Format/Review/View entries removed) and its tab-select + collapse are live signals. ✅ F7a — `AtHomeTab` reads `use_breakpoint()` (Compact = stacked, Medium/Expanded = side-by-side) instead of the fixed `viewport_width = 375.0`. ✅ F7b — slide thumbnails/bullets use stable keys (`SlideId`, shape+para) and slide deletion clamps `active_idx` explicitly. ✅ F7c — live word count in loki-text's status bar (`editing/word_count.rs`: streaming counter, Word-matching semantics — tables counted, notes excluded; 8 tests; plural `editor-word-count` key). ✅ F6d-zoom (2 of 3 apps) — the status-bar zoom badge cycles 50–200% (`appthere_ui::next_zoom`): loki-text scales the GPU page tiles + paint transform together (`DocPageSource::zoom` → `DocumentViewProps::zoom` → `PageTile` hit-test divide-out; reflow unaffected by design), loki-presentation scales the slide box + text; **spreadsheet grid-zoom deferred** (`TODO(zoom)` — needs zoom-aware `col_px` + resize px↔pt in one pass). Ceiling wins: `document_view.rs` split (`view_types.rs`) resolved its baseline entry, plus `editor_save.rs` extracted from the spreadsheet (baseline 34 → 32). **Remaining tail:** F7a measurement plumbing for Calc/Slides (they never push a measured width, so `use_breakpoint` falls back to Expanded there) + spreadsheet grid-zoom. | M |

---

## Phase 5 — Rendering & format fidelity backlog (P2, ongoing)

Locally-actionable fidelity items from §2 and §5 (upstream-gated ones are in
the Watch list). Every task here must update `docs/fidelity-status.md`.

| Task | Source | Detail | Effort |
|---|---|---|---|
| 5.1 | `tab-default` | Honour `DocumentSettings.default_tab_stop_pt` instead of the hardcoded 36 pt (`para.rs:648`). Pairs naturally with 1.1 (`tab_stops` round-trip). | S |
| 5.2 | `underline-style` / `strikethrough-style` | Double/Dotted/Dash/Wave underline, double strikethrough (all render Single today). Decoration geometry is ours (drawn in `loki-vello`), not Parley-gated. | M |
| 5.3 | `spell-baseline` | Tighten squiggle to the run underline offset (`para.rs:1619`). | S |
| 5.4 | `list-picture-bullet` | Picture bullets (fallback is `•`) — image plumbing already exists for block images. | M |
| 5.5 | `pdf-rotate` | Rotation transform in PDF export (`pdf/src/page.rs:83`); unlocks the "PDF clip/rotate paint" registry row. | M |
| 5.6 | gap #12 / `floating-image` | External-URL images render a grey placeholder (`loki-vello/src/image.rs:34`) + detect "floating" class for inline images (`resolve.rs:705`). | M |
| 5.7 | `odf-master-page` | ODF master-page transitions (`odf/reader/styles.rs:200`); pairs with the `style:default-style` registry row. | M |
| 5.8 | `omml` | OMML↔MathML: delimiters, n-ary, matrices, accents (`docx/omml/mod.rs:20`). | L |
| 5.9 | gaps #23–#30 tail | ~~Kerning~~ (✅ **#23 done 2026-07-05**: root-caused by the Phase 3 calibration pass — loki kerned unconditionally while Word/LO default off; `CharProps.kerning` now drives a shaper feature toggle with reference-matching default, regression-locked, all three visual goldens green), orphan/widow control, `border_between`, DocxSettings, content controls, language tags — schedule individually from the fidelity registry; orphan/widow is the highest-value (visible in any multi-page doc). | L (aggregate) |
| 5.10 | registry | Page/column geometry set: even/odd blank pages, unequal column widths, column height balancing; PDF font subsetting + ICC/CMYK; EPUB math/fields/comments. | L (aggregate) |
| 5.11 | `link-click` | Interactive hyperlink hit-testing (visual hint only today) — spans layout (`resolve.rs:689`, `items.rs:125`, `para.rs:203`) and renderer (`scene.rs:519`). | M |

---

## Phase 6 — Performance & memory (P2, M–L)

From audit §5 (memory-audit + perf tails) and §2. Re-measure with the
`loki-bench` harness before and after each item — do not fix unverified perf
findings (P-3/P-5/P-6 were "not re-driven").

| Task | Source | Detail |
|---|---|---|
| 6.1 | memory F3 | Drop preserved layout for inactive tabs (`sessions.rs:39` retains `Arc<PaginatedLayout>`); coordinate with Spec 06 BM-8 (inactive-tab layout retention policy) so one design serves both. |
| 6.2 | memory F5 | Share the render `FontDataCache` across tiles (per-tile `page_paint_source.rs:53` vs shared `DocPageSource`); same item as Spec 06 BM-9 (per-tile font-byte dedup). |
| 6.3 | `split-optimise` | Y-range item filter to avoid GPU clipping (Option B; Option A shipped) — `para.rs:409`. |
| 6.4 | `partial-render` | Viewport clipping / direct `node.scroll_offset` (`scene.rs:148`, `editor_pointer.rs:139`) — partially gated on the vendored blitz-dom scroll API; do the locally-possible part. |
| 6.5 | audit P-3/P-5/P-6 | Re-measure first (bench), then fix if confirmed: glyph-run scans, coarse cache invalidation, cold-path clones. |
| 6.6 | Spec 06 tails | BM-3 render-cost proxy executes once Phase 3.3 lands `vello_cpu`; GPU frame-time (`device` feature) and on-device RSS recalibration remain device-gated — keep deferred, tracked in Spec 06. |
| 6.7 | F5 residual (audit §9) | Replace the hardwired-`false` `DocumentViewProps::eq` (`document_view.rs:143-147`) with a real comparison — benign today (PageTile eq caps the cost) but pure over-render. |

---

## Phase 7 — Code-quality debt (P2/P3, ongoing ratchet)

| Task | Source | Detail |
|---|---|---|
| 7.1 | Q-1 / §6 | **300-line split pass**: burn down the 35-file baseline, starting with the files other phases must touch (`para.rs` 1979, `flow.rs` 1953 — both also carry Phase 4/5 TODOs) so the split happens *before* feature work grows them further. Use the two established techniques (inline-test extraction, directory split); update the baseline with `--update` per split. |
| 7.2 | Q-2 | App-shell duplication: extract the common per-app `routes/` + `shell.rs` scaffolding into `loki-app-shell` (it already hosts `android_main!` and `SpellService`). |
| 7.3 | Q-3/Q-4 | Writer error-swallows (301 `let _ =`) and the ~100 `#[allow]` (incl. 32 `dead_code` in OOXML): ratchet, don't big-bang — add a CI count-ratchet script (same pattern as the file ceiling) and reduce opportunistically when touching a file. |
| 7.4 | audit S-1b/S-2/S-3/S-5 | Security tails: nested-table drop, dimension clamp, UTF-16 odd byte, XXE comment. Small, parser-local; batch into one hardening pass with fuzz-style regression tests. |
| 7.5 | T-2/T-3/T-5 | Test tails: ODT export coverage, per-case DOCX/XLSX round-trips, hard PPTX cases — grow alongside Phase 3's conformance corpus rather than as a separate effort. |
| 7.6 | Spec 01 residuals | `clippy::pedantic` + allow-list, `no_hardcoded_layout_dims` dylint, `cargo udeps` dead-`pub` sweep, Android target build verification in CI. Deliberate residuals; schedule after 7.1 has reduced churn. |

---

## Watch list — upstream-gated, not schedulable (re-check on every dependency bump)

No local work can close these; the action is to **re-verify the removal
condition whenever the pinned dependency moves** (per the "Upgrading Dioxus"
procedure in `docs/patches.md`).

- **The five Dioxus/Blitz patches** (`dioxus-native`, `dioxus-native-dom`, `blitz-shell`, `blitz-net`, `blitz-dom`) — each has its removal condition in `docs/patches.md`; none met as of 0.7.9 / 0.2.x.
- **Parley**: native `BaselineShift` (super/sub is already visually correct via the manual offset — S-3), bidi/RTL direction API (gap #19), inline image boxes (`inline-image-flow`).
- **Vello**: blur primitive for true soft text shadow (`shadow` — hard offset copy today).
- **Blitz CSS**: `white-space: nowrap`, `text-overflow: ellipsis`, `:hover`, `scrollbar-width`, SVG rendering, `position: fixed` (collapses to `absolute`) — runtime-verify each on every Blitz bump and update the `CLAUDE.md` confirmed/unconfirmed lists.
- **Platform quirks (permanent)**: Mali-G715 Vulkan device-loss workarounds, Android 16 double `ANativeActivity_onCreate`, Word/OOXML file quirks — documented COMPAT, never removable.
- **Patch-tree fixes queued for the next re-vendor** (audit §9 F7e/F7f): strip the `[LOKI/head]` / `println!` / `dbg!` debug leftovers from the vendored patches, and fix the `buttons ^= Main` XOR on touch end/cancel (`patches/blitz-shell/src/window.rs:1133`, should be `&= !Main`) — batch these with the next Dioxus/Blitz patch re-vendor rather than churning the vendored tree now.
- **Headless/server deferrals with in-code TODOs**: `TODO(headless-c025)` (apalis job queue), `TODO(headless-c021/c022/c023-discovery/c027/c028)`, `TODO(kms)`, `TODO(ws-membership)` — deliberate spec deferrals (ADR C021–C028); schedule when the server milestone that needs them is planned, not before.

## Out of scope — deliberate post-MVP (do not schedule)

Audit §7: Loki Calc / Loki Slides post-MVP items (virtualization >500×52,
richer formulas, charts, PPTX image/group export, masters/layouts, ODP import,
shape editing, etc.) remain governed by
`docs/mvp-scope-spreadsheet-presentation-2026-06-13.md`. Two exceptions worth
pulling forward when either app is next touched, because they violate suite
conventions rather than MVP scope:

1. **i18n bypass** — both apps hardcode user-visible strings, against the "never hardcode" rule; migrate to `fl!()` opportunistically.
2. **Zero tests** — add smoke tests for whatever Phase 0 confirms from F1/F2 (edit loss, no-op delete/copy) before fixing them.

---

## Suggested sequencing

```
Now (parallel):   Phase 0 (verify F1–F7)   Phase 2 (loki-file-access)   Task 7.1 (split para.rs/flow.rs)
Next:             Phase 1 (CRDT integrity) Phase 3 (conformance foundation)
Then:             Phase 4 (editor completion, informed by Phase 0)
Ongoing ratchet:  Phase 5 (fidelity), Phase 6 (perf, bench-gated), Phase 7 (quality)
Every dep bump:   Watch list re-verification
```

Rationale for the front of the queue: Phase 0 is cheap and de-risks Phase 4;
Phase 2 is the only patch we fully control and shrinks the patch surface;
splitting `para.rs`/`flow.rs` first prevents every later phase that touches
them (5.1–5.4, 4b.5, 6.3) from fighting the ceiling ratchet; Phase 1 protects
user data; Phase 3 builds the verification infrastructure that keeps Phases 4–5
honest.
