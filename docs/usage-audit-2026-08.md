# Loki Text usage observations — investigation and plan (2026-08-10)

Source: user usage notes ("Loki Text Writing Experience", written before the
Loki Text Dialogs design work landed). Each observation was investigated
against the tree at the dialogs branch head; this document records what is
**Observed** (with evidence), what is **stale** (already fixed by the dialogs
work or since), the **gaps**, and a sequenced plan. Two observations from the
notes — paragraph formatting and metadata as proper modal dialogs — are
delivered by the dialogs patch (design sections 1 and 4) and are not repeated
here.

Numbering follows the order of the notes. §B lists defects *discovered during*
the investigation that the notes did not ask about; §P is the proposed
sequencing.

---

## §1 UI too large on small laptops (11″ / 1366×768)

**Observed.** All UI tokens are compile-time `f32` consts consumed via
`format!` at ~1400 sites (`appthere-ui/src/tokens/`); no scale factor exists.
The one adaptive size is `tab_strip_height(breakpoint)`
(`ribbon/tab_strip.rs:21-27`). Breakpoints are width-only (Compact <600,
Medium <1024, Expanded) — a 1366×768 laptop and a large tablet are both
"Expanded". Pointer precision (`PointerPrecision`) is modelled but produced in
exactly one component and consumed nowhere that sizes anything.

**Three facts complicate the requested "detect low density, shrink 20%":**

1. *A text-scale pass is already specified* — Spec 08 I-24. The token header
   (`tokens/spacing.rs:9-36`) fixes the rule: dimensions are split into
   text-relative vs device-fixed, combined as `max(text-relative,
   device-fixed)`; `TOUCH_MIN = 44` is device-fixed and must not scale. A
   blanket −20% would violate the written rule; the compliant form scales the
   text-relative class only. A live gate row
   (`scripts/pending-questions.txt:52`) already tracks the missing
   `device_profile` `font_scale` field.
2. *Density is only readable on X11, and only after first paint*
   (`loki-app-shell/src/display_probe_x11.rs`; Wayland/macOS/Windows probes
   are explicitly not written; the probe polls until the first tile paints).
   The display-calibration store (`display_calibration.rs`, keyed by display)
   is the natural place to persist a last-known value so launch isn't blind.
3. *PPI inverts the heuristic.* An 11″ 1366×768 panel is ~125 ppi — higher
   than a 24″ 1080p desktop (~92 ppi). The signal that actually separates the
   complaint's device is **physical size** (px ÷ ppi), which the X11 probe has
   inputs for but nothing computes.

**Plan.** Implement I-24 as specified rather than an ad-hoc shrink: add
`ui_scale`/`font_scale` to `DeviceProfile` (fed by physical-size-derived
default, user-overridable in settings), convert text-relative tokens to
accessors honouring the `max()` rule, keep `TOUCH_MIN` fixed, persist the
scale per display. Pre-req: decide the mechanism (token accessors vs root
`rem` — `rem` is unverified in Blitz and CI forbids font-relative units in
`editor_canvas*.rs`, so token accessors are the safer first step). This is a
medium-sized cross-cutting change; the pending-questions register names what
re-opens when it lands (`MENU_ROW_HEIGHT_PX`, popover reachability).

## §3 Layout tab: named page styles, paper sizes, margins, columns

**Substantially stale for geometry; accurate for named styles.** The new Page
style dialog (design section 3) already provides: the full 28-entry paper
catalogue + custom width/height (`page_dialog/tab_page.rs`), free-form margins
with gutter and mirroring (`tab_margins.rs`), column gap + separator
(`tab_columns.rs`), headers/footers, page borders. The ribbon's
Letter/A4-only and 3-margin-preset restrictions are deliberate; the dialog is
the escape hatch.

**Still missing (all model-complete, UI-absent):**
- **Assign/create/rename/duplicate/delete named page styles** — every verb
  exists (`loro_mutation/page_style_assign.rs`, `page_style.rs`) and has UI in
  the style catalog editor panel — but that panel **lost its only opener**
  when the dialogs patch rewired the Write tab's Paragraph button
  (`editing_style_draft` is never set to `Some` by anything reachable).
  Collateral: the measurement-unit picker and page-defaults rows in the same
  panel are also unreachable, pinning the dialog's unit to the locale default.
- Column count >3 and per-column widths (model + layout + both importers
  support unequal widths end-to-end; no UI writes them, and the dialog's
  count-change handler drops imported widths).
- No section-break mutation exists at all, so multi-section documents only
  arise from import — named styles per section only matter for imported files
  until one is added.

**Plan.** (a) Restore an opener for the style catalog editor (a Manage styles
button; natural home: Layout tab or the paragraph dialog's breadcrumb). (b)
Add a page-style picker + "apply to section" to the Page dialog's Page tab.
(c) Extend `tab_columns.rs` to a stepper (model supports ≤12) + per-width
fields, preserving imported widths. (d) `insert_section` mutation as its own
work item. Two defects found here are listed in §B.

## §4 Write tab Document group: New / Open / Save split buttons

**Observed.** The group today is Save / Save As / Save as Template
(`editor_ribbon.rs:84-118`); no New, no Open, no Ctrl+N/O. Every underlying
capability already exists and is reachable from ribbon context: blank/template
tab creation (`new_document.rs`), the file picker (`home.rs:74-114`), the
recents store as an app context signal (`app.rs:159-168`, capped 20), tab
push/switch helpers, and save callbacks — including the exact requested Save
semantics (Save routes untitled documents to Save As already,
`editor_save_callbacks.rs:180-184`).

**Missing:** a split-button component (zero exist in appthere-ui — the
anchored-popover machinery and two row-menu consumers are the building
blocks); "Save a Copy" (structurally `use_save_as_callback` minus the
tab-repoint/recents/navigate lines); New/Open icons + ~10 i18n keys; and the
template menu should read `loki_templates::TEMPLATES` instead of duplicating
the id list (today `home.rs:55-59` hardcodes it — second source of truth).
The unused `AtTemplateBrowser` component is available for the New menu.

**Plan.** New `appthere-ui` `ribbon/split_button/` component (main action +
anchored menu, reusing the overflow-menu popover pattern); new
`editor_ribbon_document.rs` module for the group (editor_inner may not grow);
`estimate_group_metrics` must reflect the new control count. Straightforward,
self-contained feature.

## §5 Release packaging (MSI, DMG, deb/rpm/Flatpak, Android/iOS)

**Observed: greenfield except Android.** Desktop packaging is three
`[package.metadata.bundle]` stubs; no bundling tool, no release CI (the only
workflow has no tag trigger, no Windows/macOS runners, no artifact upload),
and **zero packaging assets** — no app icons in any format, no `.desktop`
files, no AppStream metainfo, no Info.plist, no MIME registrations. Android
has a working cargo-apk + manifest-injection pipeline, but the `.sh` is
hardcoded to loki-text while `build-android.ps1` is already parameterised over
all three apps; the Gradle/AAB path is loki-text-only; `android/app/src/main`
has no `res/` (default system icon). iOS has nothing but a README paragraph.
App identity is inconsistent (`com.appthere.loki` vs `.loki.text` vs
`.loki.calc`) and will be forced by packaging.

**Plan (ordered).** (1) Assets first — icons, `.desktop`, metainfo, MIME
declarations; blocks every format. (2) Reconcile bundle identifiers. (3)
cargo-packager for MSI/DMG/deb/rpm (reads the cargo-bundle-shaped metadata
already present) + a tag-triggered release workflow with a 3-OS matrix. (4)
Flatpak as its own track (`flatpak-cargo-generator` must be taught the
`patches/` overrides + git deps). (5) Parameterise `build-android.sh` /
`build-aab.sh` over the three apps as the ps1 already does; launcher icons.
(6) iOS harness (Xcode template wrapping the cdylib) as its own project —
nothing in-repo constrains the choice yet.

## §6 Print

**Observed.** No print path exists in any app: no button, no Ctrl+P, no
system-dialog integration on any platform; winit/Blitz offer none natively.
`loki-print` is a headless IPP client consumed only by the `loki-headless`
CLI. The document side is solved: `loki_pdf::build_pdf` explicitly accepts the
editor's already-computed layout, and `editor_publish.rs::serialize` produces
the bytes today.

**Plan.** New capability crate mirroring `loki-file-access`'s per-platform
shape (`rfd`-style portal on Linux — `org.freedesktop.portal.Print` keeps
Flatpak working; `NSPrintOperation` on macOS; `PrintDlgEx`/WinRT printing on
Windows; `PrintManager` over the existing JNI trampoline pattern on Android;
`UIPrintInteractionController` on iOS when the harness exists). Publish-tab
Print button + `dialogs.print` signal + Ctrl+P; render via `build_pdf` from
the live layout; optionally keep an "IPP printer URI" path reusing
`loki-print` for office deployments. `PrintOptions` lacks page-ranges — add
before UI. Spreadsheet/presentation printing is blocked on their nonexistent
PDF export and should be scoped out initially.

## §7 Template modifications

**Observed.** Templates live in `loki-templates` (Rust builders → generated
`.dotx` assets, re-imported at runtime; builders may only use round-trippable
properties). Findings against the four requests:

1. *Screenplay title page*: none exists; the template cannot currently express
   one — the builder DSL has no `page_break_before` and hardcodes a single
   section. Model supports both. Fountain's native title-page block (§12)
   wants the same capability — do the template work first.
2. *Courier Prime 12pt*: the template is already uniformly 12pt but names
   **Courier New**; the only bundled monospace is **Cousine**. Courier Prime
   (OFL) is not in `loki-fonts`. Adding it: font files + two registry lists +
   the parity test arithmetic.
3. *Bundled fonts only*: **every template currently violates this** — all five
   name proprietary families (Arial/Times New Roman/Courier New), which
   resolve through the substitution table and therefore raise the
   "fonts substituted" chip on machines without MS fonts. Renaming builder
   fonts to Arimo/Tinos/Cousine (or Courier Prime) fixes the chip and the
   guarantee. MLA/APA specify "Times New Roman or similar" — Tinos qualifies.
4. *Next paragraph style*: **largely authored already** — every template sets
   `next_style_id` (Screenplay already has Character→Dialogue,
   Transition→SceneHeading). The reason it doesn't work is a **code bug**, not
   template data: see §B1. Two template-data fixes remain: Dialogue→Dialogue
   should be Dialogue→Action, and Normal→None triggers style loss on split.
5. *Blank = Markdown minus sample text*: today "Blank" bypasses
   `loki-templates` entirely (`Document::new_blank`, headings only, no body
   font, no next-styles). Cheapest correct change: add a `blank` template
   derived from markdown's catalog with an empty body, route card 0 and the
   shell's `+` to it — but the Blank arm is the only one that applies user
   page-size/margin defaults (`editor_load.rs:63-73`), so the template arm
   needs a special case to preserve that.

## §8 Font-substitution notification persists across tabs (bug)

**Mechanism found.** The substitution map lives on `FontResources`
(`loki-layout/src/font.rs:36`) inside the app-root `SharedFontResources`
context — process lifetime — and doubles as `resolve_font_name`'s memo cache;
nothing anywhere clears or removes entries. Tab switch resets the panel-open
signal (why the panel closes) but the chip reads the global map. Only an app
restart clears it. Discriminating prediction confirmed by inspection: closing
every tab cannot clear the chip.

**Fix shape.** Split the two roles: keep the resolve memo, record the
*reportable* substitution set per layout run and store it per-document
(`DocumentState`), intersect for display; clear on the `editor_path_sync`
reset/restore paths; give the chip a reactive source (same pattern as §9's
fix). Medium-small, in loki-layout + loki-text.

## §9 "Page 2 of 1" status bar (bug)

**Mechanism found.** `current_page` is written by the scroll handler against
the *live* `doc_state.page_count`; `total_pages` is a Signal mirror written on
only four paths — open, load-effect, restore, reset — **never on the edit
path** (`apply_mutation_and_relayout` updates `page_count` behind the mutex;
no signal write). This is the mirror-image of the already-diagnosed I-10
word-count bug, whose fix pattern (`use_word_count_label` keyed on the
mirrored `document_generation`) is the template. Predictions: self-heals on
tab switch; inverse staleness ("Page 1 of 2" after deleting) reproduces; the
custom scrollbar shares the stale value.

**Fix shape.** Replace the load-keyed effect with a `document_generation`-keyed
memo/effect setting `total_pages` from `page_count` (single point, no edits to
~28 mutation call sites). Small.

## §10 Lists

**Observed.** The model is *complete* (ListStyle catalog: 9 levels, all seven
numbering schemes, `%N` multi-level formats, per-level bullets/indents/fonts,
named custom styles) and the layout path for `StyledPara` + `list_id` renders
all of it today (the 2026-04 audit's five steps all landed). What's missing is
everything around it:

- **No way to create a list**: no `set_block_list` mutation (only `get`/
  `clear`), no ribbon buttons, no icons, no autoformat, and **Tab is
  unreachable** — the blitz-dom patch intercepts it for focus traversal before
  dispatch (`patches/blitz-dom/src/events/keyboard.rs:25`).
- **Export destroys lists**: DOCX write emits `w:numPr` only for legacy
  pandoc list blocks and writes `StyledPara.list_id` as a plain paragraph;
  nine-level definitions are flattened to one hardcoded level. ODT write emits
  bare `<text:list>` with no style. **Any list-creation UI shipped before the
  writers is a data-loss feature.**
- **Two representations**: ODT import still produces the legacy pandoc blocks
  (level-0 attributes only, hardcoded `•`/`1.` at fixed 18pt, and — because no
  `PathStep` addresses list items — *ODT-imported lists are read-only*). The
  2026-04 audit already recommended converging ODT import onto
  `StyledPara`+`list_id`; that is the single largest simplification.
- Style panel's list family is a read-only browser (no draft, no commit fn,
  no per-level form).

**Plan (dependency order).** (0) blitz-dom Tab patch (dispatch before focus
fallback). (1) `set_block_list`/`set_block_list_level` mutations (copy the
`align.rs` shape) + built-in default bullet/numbered `ListStyle`
constructors. (2) **DOCX/ODT list writers** (real 9-level serialisation;
`w:numPr` for styled paras) — ships *with*, not after, the UI. (3) Ribbon:
bullet/number/indent± buttons + icons + keys; Tab/Shift-Tab keyboard arm.
(4) ODT import convergence onto path A (retires the read-only path). (5)
Per-level list-style editor form in the style panel. (6) Refinements: label
alignment, `lvlText` restarts, `"- "` autoformat (greenfield).

## §11 Inline formatting overflow on small screens

**Observed.** The collapse engine is whole-group only (`Full → Condensed →
Overflow`; condensed just drops label/padding — never buttons). The group is
declared as one opaque `Element` with a single width; there is no per-control
model. A close precedent exists: the status bar's per-item priority model with
a `retained` set (`status_priority.rs`) is exactly "B/I/U always visible,
rest droppable", one level down. Tension to resolve: the inline group has the
*highest* priority on the Write tab (last to shrink), so partial overflow is
an opt-in phase, not a priority tweak.

**Plan.** Add an optional per-item spec (`RibbonItemSpec { retained, … }`) to
`RibbonGroupSpec`, a `Partial` collapse state + third width, and a per-group
submenu popover (respecting the popover singleton rule and the known
Tab-dismiss limitation). Medium; touches the cascade tests. Reasonable to
defer until after §14's one-liner and the density work (§1), which may change
the constants underneath.

## §12 Markdown and Fountain import

**Observed.** No Markdown parser exists anywhere (the "Markdown template" is
styling + sample prose only); Fountain has zero hits in the workspace. Import
dispatch has two seams: the app's `detect_format`/`import_token`
(`editor_load.rs`) and the headless `loki-convert` matrix (ADR-C024).
Notably, **the screenplay template's style ids map 1:1 onto Fountain's
element grammar** (SceneHeading/Character/Parenthetical/Dialogue/Transition),
making the "into their respective templates" requirement unusually clean; the
same holds for markdown→Heading/Blockquote/CodeBlock.

**Plan.** Two new leaf crates per the one-crate-per-family rule
(`loki-markdown` with pulldown-cmark, `loki-fountain` hand-rolled — the
grammar is small), each implementing `DocumentImport`, emitting blocks against
the corresponding template's catalog (decide: importer depends on
loki-templates, or caller merges the catalog — the latter matches how
ooxml/odf behave). Wire into `detect_format` + MIME table (subset-invariant
test constrains where) + `loki-convert` matrix as import-only sources (the
existing EPUB/PDF export-only shape, inverted). Fountain title-page mapping
depends on §7's title-page capability. No export, per the notes.

## §13 Open from file manager / intents / single instance

**Observed: nothing exists.** No argv handling in any app (`std::env::args`
is never read); the app always boots to Home. **Gating constraint**: an editor
tab requires a serialised `FileAccessToken`, and only the picker backends can
mint one — there is no public `from_path`, so even a parsed argument can't
become a tab without extending the vendored `loki-file-access` patch. No
single-instance mechanism of any kind. Android manifests declare only
MAIN/LAUNCHER — no VIEW intent-filter, no MIME data, default `launchMode` (and
the manifest exists in two places that must be edited together). iOS is
blocked on the §5 harness. Desktop associations need the §5 packaging assets
(`.desktop` MimeType, `CFBundleDocumentTypes`, registry) — none exist.

**Plan (layered).** (1) `loki-file-access`: public `FileAccessToken::
from_path` (desktop) / `from_content_uri` (Android) — patch + upstream. (2)
argv → stash before launch → seed tabs/route instead of Home. (3) Android:
VIEW intent-filter (MIME strings already exist in `home_templates.rs`) +
`launchMode="singleTask"` + intent handling in `android_main` — this *is*
Android's single-instance answer. (4) Desktop single-instance forward (socket
probe before launch; receiver lands on the existing `open_or_switch`; also
fixes the recents-file clobbering between instances). (5) The association
declarations ride on §5's packaging assets.

## §14 Collapsed ribbon: tab click should expand

**Observed.** Collapsed merely unmounts the content row; tab clicks move the
indicator and nothing else. The strip already receives both `collapsed` and
`on_toggle_collapse`, so the fix is a behavioral change in `AtRibbon` (expand
before forwarding the index) — fixing loki-spreadsheet's identical copy for
free, per the crate's "framework owns the cascade" doctrine. Decide: click on
already-active tab while collapsed also expands (recommend yes). Small; a new
regression test.

## §15 Memory

**Prior art established** texture residency is document-length-independent and
budget-bounded; inactive tabs stash no layout; the >3GB idle loop and the
per-scroll texture leak are fixed by local patches. **Four ranked mechanisms
explain the reported profile** (each with a cheap discriminating measurement;
none yet run — this environment has no GPU/display):

- **A1 — `ParaCache` grows one entry per keystroke** (key includes paragraph
  text; 2×2048 *entries*, no byte bound; entries own the glyph data; cleared
  only on next document open). Derived ceiling 20–270 MiB (3× CJK). Also the
  direct explanation for **memory staying high after closing all tabs**: the
  cache lives on the app-root `SharedFontResources`. *Settle it:* log
  `para_cache.len()` + estimated bytes in the existing `loki_text::mem`
  counter; falsify with a `CACHE_CAP=64` build.
- **A2 — Loro history cache built per keystroke, freed only at save**
  (`IncrementalReader::update` calls `loro.diff` every mutation → detached-
  mode history cache; the single `free_history_cache` site is on the save
  path). *Settle it:* log `has_history_cache`; measure RSS step-down at save.
- **A3 — Vello image-atlas ratchet**: every registered page texture is copied
  into a power-of-two atlas **every frame**; the atlas doubles up to 8192² and
  never shrinks; the atlas texture is recreated per frame (upstream TODO).
  Roughly doubles the budgeted texture bytes and is invisible to RSS-based
  audits — the "driver-side overhead" the Spec 08 baseline lists as
  not-established. *Settle it:* read the existing-but-never-read
  `TextureResidency::snapshot()` beside RSS; test with `LOKI_TEXTURE_BUDGET_MB
  =24`.
- **A4 — allocator retention** from per-keystroke full `Document` clones +
  layout churn (glibc arenas hold the high-water mark; zero leaked bytes under
  heaptrack). *Settle it:* heaptrack peak-vs-RSS; `MALLOC_ARENA_MAX=2` run.

**Windows/macOS higher baseline**: dual/triple system-font scans + ~16 MB
embedded-font copies are platform-neutral; the standout suspect is **budget
derivation asymmetry** — the memory probe uses different sources per platform
(`sysinfo` vs `/proc/meminfo`, explicitly "verified by type-check, not on
device"), and a larger Windows "available" figure derives a deliberately
larger texture budget from the same hardware (tracked as I-25). *Settle it:*
compare logged `BudgetSource`/`budget_bytes` across platforms on equal-RAM
machines.

**Also corrected here:** two documents assert `max_undo_steps(100)`; **no code
sets it** — the undo stack is unbounded with per-keystroke items
(`UndoManagerInner::new` defaults). Bounding it is *not* a plain one-liner:
the dirty-indicator checkpoint logic documents that it relies on no
merge-interval/grouping, and stack eviction bypasses the `on_pop` mirror.

**Plan.** Phase 1 (instrumentation, cheap): the three loggers above + the
heaptrack scenario script. Phase 2 (act on what the numbers say), likely:
byte-bound + close-time clear for `ParaCache`; periodic
`free_history_cache`/compaction between saves (respecting the saved-state
coupling); revisit the r15 survival ceiling if A3 confirms; jemalloc/mimalloc
or `MALLOC_ARENA_MAX` guidance if A4 dominates. Phase 3: fix the two stale
docs (done in this audit's follow-up), add a typing-workload bench that does
*not* pre-clear the cache (the current one deliberately excludes it).

---

## §B Defects discovered during investigation (not in the notes)

| # | Defect | Where | Severity |
|---|---|---|---|
| B1 | **Enter-key next-style lookup uses display keys** ("Heading 1", "Default Paragraph Style") against catalog ids ("Heading1") — so `next_style_id` never fires for headings or plain paragraphs; same bug class the paragraph-dialog fix (`editor_style_target::resolve_target`) already solved, ready to reuse. Two adjacent defects: split keeps the heading block type (needs `set_block_type_para` when next style isn't a heading), and next-style applies regardless of caret position (should be end-of-paragraph only). | `editor_keydown_enter.rs:84-109` | High — defeats §7(4) entirely |
| B2 | Page dialog reads geometry only from the catalog copy, which the ribbon's document-wide mutations never update — the dialog can show and **re-commit stale pre-ribbon-edit geometry** | `page_dialog/body.rs:29-36` vs `loro_mutation/page.rs` | Medium |
| B3 | Metric-compatible substitution list omits Arimo/Cousine/Tinos, so Loki's own bundled substitutions are badged "approximate" | `editor_font_warning.rs:36-47` | Low |
| B4 | `page_style_group` shares `priority: 0` with the Columns group; collapse order unspecified against the stated intent | `editor_ribbon_layout.rs:282` | Low |
| B5 | Stale docs: `memory-audit-2026-06-12.md:145` and `spec-06-benchmarking-audit.md:68` assert an undo bound no code sets | docs | Low (doc-only, but per L08-017 it keeps being believed) |
| B6 | `AtRibbonSelect` doc comment still claims floating overlays are impossible (stale since the popover host) | `ribbon/select.rs:43-48` | Trivial |
| B7 | Split of a styled paragraph drops `style_id` on the tail block (copies type/props but not the style reference) | `loro_mutation/block.rs:111-152` | Medium — silent style loss on Enter |
| B8 | ODT import hardcodes `next_style_id: None` (round-trip loses it) | `loki-odf/src/odt/mapper/styles.rs:53,146` | Low |

## §P Proposed sequencing

**Now (bug fixes, small, high confidence):** — **all five landed 2026-08-10**
(same branch as this audit; each with the inversion tests the fix admits).
1. §9 total-pages mirror (I-10 pattern) · 2. §B1 Enter next-style resolution
(reuses the resolver pattern; new read-only `editor_next_style`) + B7 (+ a
related fix: `set_block_type_heading` now drops a stale stored
`heading_style`) · 3. §14 ribbon expand-on-tab-click (in `AtRibbon`, so
loki-spreadsheet inherits it) · 4. §8 per-document substitution reporting
(per-run recording on `FontResources`, accumulated on `DocumentState`,
cleared on load/switch) · 5. §B3, §B4, §B6, §B5 sweep.

**Next (small features / stale-observation completions):**
6. §3(a) restore the style-manager opener + §B2 · 7. §7 template pass
(Courier Prime bundling, bundled-fonts-only rename, Dialogue→Action,
blank-from-template, title-page capability) · 8. §4 Document group split
buttons · 9. §15 Phase 1 instrumentation (unblocks the memory decisions).

**Then (medium tracks):**
10. §10 lists tiers 0–3 **with** the export writers · 11. §6 Print
(desktop portals first) · 12. §12 Markdown+Fountain import ·
13. §1 density/text-scale (I-24) · 14. §15 Phase 2 per the numbers.

**Later (large/independent tracks):**
15. §5 packaging (assets → cargo-packager → CI matrix → Flatpak → iOS
harness) · 16. §13 file associations + single instance (layered on §5) ·
17. §10 tiers 4–8 (ODT convergence, per-level editor) · 18. §11 partial
group overflow.
