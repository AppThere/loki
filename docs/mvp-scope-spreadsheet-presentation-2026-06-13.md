# MVP Scope — Loki Spreadsheet & Loki Presentation (2026-06-13)

What each app needs to reach a usable MVP — "open a real file, edit it, save it
back without losing data." Based on source verification of `loki-spreadsheet`
and `loki-presentation` and the crates they depend on.

**Headline:** the two apps are at very different maturity levels.
- **Spreadsheet** is ~80% of an MVP: it already imports/exports XLSX **and** ODS
  through tested importers, edits via a Loro CRDT, and saves. The gaps are
  finite and mostly UX/feature breadth.
- **Presentation** is a UI prototype: an HTML/CSS slide editor over an in-memory
  demo deck with **no document model, no file format, and no persistence**. It
  needs foundational infrastructure built before it can open or save anything.

---

## 1. Loki Spreadsheet

### Current state (verified)

| Area | Status |
|---|---|
| New / Open / templates | Works — `home.rs` (blank + template tabs, `pick_file_to_open`) |
| Import | **XLSX + ODS**, real & tested (`XlsxImport`, `OdsImport`; loki-ooxml has 160 tests) |
| Editing | Cell value + formula edits through Loro (`mutate_cell`); bold/italic/underline/align/number-format via `mutate_cell_style`; undo/redo wired |
| Formula bar | Works (select cell → edit `=…` in the bar) |
| Save / Save As | Works — `save_document` exports XLSX/ODS; untitled routes to `pick_file_to_save`; updates tab + recents |
| Cell selection | Click-to-select; row/col/cell highlight |

### MVP gaps (prioritised)

1. **~~Grid is hardcoded to A1:J30~~ — DONE (this branch).** The grid now follows
   the workbook's used range plus padding, clamped to a render-friendly cap
   (`cell_ref::grid_dimensions`), with bijective column labels (A…Z, AA…,
   `col_to_label`) and a generalised `parse_cell_ref` supporting arbitrary
   multi-letter columns and full-height rows (bounded by the XLSX limits). Data
   outside the old 10×30 window is now visible and editable.
   - *Remaining follow-up:* true **row/column virtualization** so sheets larger
     than the cap (currently 500×52) render fully without a DOM-size cap.

2. **~~Formula engine is `SUM` + naive `+`/`-`~~ — DONE (this branch).** Replaced
   the hand-rolled scanner with a recursive-descent evaluator
   (`formula/{lexer,eval}.rs`): operator precedence, parentheses, unary minus,
   `+ - * /`, the functions `SUM`/`AVERAGE`/`MIN`/`MAX`/`COUNT`/`IF`, A1 ranges,
   and Excel-style error values (`#NAME?`/`#VALUE!`/`#DIV/0!`/`#REF!`/`#NUM!`)
   with error contagion through references instead of a silent `0`. Covered by
   31 unit tests (`loki-spreadsheet` went from **0 → 31 tests**).
   - *Remaining follow-up:* more functions (`COUNTIF`, `ROUND`, text fns),
     string/boolean value types, and `<`/`>`/`=` comparisons for richer `IF`.

3. **~~Save errors are invisible to the user~~ — DONE (this branch).** `save_document`
   now threads a `save_message` signal: picker, token, open-write, and export
   failures set a localised message (`editor-save-error` / `error-file-picker`),
   and a successful save sets `editor-save-success`. A dismissible banner renders
   the message between the title bar and the grid. (Load errors were already
   surfaced via `EditorErrorView`.)
   - *Remaining follow-up:* surface the in-edit mutation-sync failures
     (`apply_change`) too; today those remain `tracing`-only.

4. **~~No keyboard cell navigation~~ — DONE (this branch).** The grid container is
   now focusable (`tabindex="0"`); when not editing, **Arrow keys** move the
   selection, **Tab/Shift+Tab** move horizontally, **Enter** moves down, **F2**
   enters edit mode, and **Delete/Backspace** clears the cell. (Runtime
   focus/key delivery depends on the blitz-dom focus patch and is pending
   on-device verification.)
   - *Remaining follow-up:* type-to-edit (begin editing on a printable key),
     and range selection via Shift+Arrow.

5. **Dead chrome:** ribbon tab selection (`on_tab_select` no-op), ribbon
   collapse (`on_toggle_collapse` no-op), and zoom (`zoom_percent` hardcoded
   100, `on_zoom_click` no-op). MVP can ship with these hidden or wired; today
   they look interactive but do nothing.

### Out of scope for MVP (post-MVP)
Charts, multiple-sheet tab UI (model supports many sheets; UI shows one),
frozen panes, cell comments, conditional formatting, find/replace, copy/paste of
ranges, column/row resize and insert/delete.

### Effort estimate
**Small–medium.** Items 1–4 are the core; none require new crates. The model
and I/O already exist and are tested. This is the cheaper of the two apps to
finish. **Items 1–4 are now done** (see status above); only item 5 (dead
ribbon/zoom chrome) remains, plus the noted follow-ups (grid virtualization,
richer formulas, type-to-edit). The spreadsheet is at or near a usable MVP.

---

## 2. Loki Presentation

### Current state (verified)

`editor_inner.rs` renders an HTML/CSS slide editor (sidebar thumbnails + a
720×405 canvas) over an in-memory `Slide` struct
(`title`, `subtitle`, `bullets`, two colours). It honestly shows a
"preview-only" banner. Critically:

- **The deck is hardcoded** (`use_signal(|| vec![Slide{…}, …])`); the opened
  file's `path` is used only for the window title. Edits are live but **purely
  in memory and discarded** on close/navigation.
- **No presentation document model** exists anywhere in the workspace (no
  `loki-presentation-model` crate; `Slide` is a private UI struct).
- **No PPTX/ODP support** — `loki-ooxml` implements only `docx` + `xlsx` (no
  `presentationml`); nothing in `loki-odf` handles `.odp`. The app does not
  reference any importer/exporter.
- **No Loro CRDT bridge**, so no real undo/redo or collaboration path.
- The crate depends on the GPU pipeline (`loki-layout`, `loki-vello`,
  `loki-renderer`) but the editor uses none of it — slides are plain HTML.

So unlike the spreadsheet, presentation is missing the entire data + I/O
foundation, not just UI polish.

### MVP gaps (build order)

1. **Define a presentation document model.** New crate
   `loki-presentation-model` (mirroring `loki-sheet-model`): `Presentation`,
   `Slide`, `Shape`/`TextBox` (title, body, bullets), basic theme/colours, slide
   size. Keep it format-neutral.

2. **PPTX import/export.** Add a `pptx` feature/module to `loki-ooxml`
   (PresentationML: `presentation.xml`, `slideN.xml`, slide layouts/masters,
   relationships). This is the largest single piece — it is a new OOXML part
   family on top of the existing OPC plumbing (`loki-opc` already handles the
   ZIP/parts/relationships, which helps). Import-first is acceptable for MVP;
   export can follow but is needed for "save without data loss."
   - *Optional:* ODP via `loki-odf` for parity with the spreadsheet's dual
     format support — can be deferred past MVP if PPTX lands first.

3. **Loro CRDT bridge** for the model (mirroring `loki-sheet-model::loro_bridge`
   and `loki-doc-model`), so editing, undo/redo, and save snapshots are
   consistent with the other apps.

4. **Wire load + save into the editor.** Replace the hardcoded `vec![Slide…]`
   with `load_document(path)` (import → model → signal) following the
   `loki-spreadsheet` `editor_load.rs` pattern, and add Save / Save As
   (`pick_file_to_save`, exporter, recents, dirty flag) following the
   `loki-text`/`loki-spreadsheet` save flow.

5. **Reconcile the rendering approach.** Decide whether MVP keeps the current
   HTML/CSS slide editor (fastest path; just bind it to the model) or moves to
   the Vello GPU canvas the deps imply. **Recommendation:** keep HTML/CSS for
   MVP and bind it to the real model + persistence; defer the GPU canvas.

6. **Fix the known editor bugs while rebuilding** (from `audit-2026-06-10.md`
   F1/F7): in-memory edits lost on tab switch; index-based slide keys
   re-associate state when a slide before the active one is deleted; dead ribbon
   tab/zoom handlers.

### Effort estimate
**Large.** Item 2 (PPTX) dominates and is a multi-week effort on its own; items
1, 3, 4 are each comparable to their `loki-sheet-model`/spreadsheet analogues.
A defensible MVP-minus could ship **import-only** (open & view/edit a real
`.pptx` in memory) before round-trip export is complete — but "MVP" as defined
here (open → edit → save back) requires export too.

---

## 3. Recommended sequencing

1. **Finish Spreadsheet first** — small surface, high payoff: dynamic grid (1),
   then formula engine + error surfacing (2, 3), then keyboard nav (4). Ships a
   genuinely usable second app quickly.
2. **Then Presentation, foundation-up:** model crate → PPTX import → bind editor
   to model + load → Loro bridge → PPTX export / Save As. Treat import-only as
   an intermediate milestone.

### Cross-cutting (both apps, also flagged in `codebase-analysis-2026-06-13.md`)
- Both bypass `loki-i18n` for many strings ("Save", "Add Slide", theme names) —
  i18n them as they are touched.
- Both have **0 tests**. The spreadsheet formula engine and the new presentation
  model/import are the natural first test targets.
