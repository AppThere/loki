<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Loki Spec 08 — Remaining Work

| Field | Value |
| --- | --- |
| Status | Phases 0–5 complete or feature-complete; **Phases 6–7 remain** |
| Source | Spec 08 r72 (full history), condensed to open items only |
| Companions | Spec 09 (layout memory, parked at boundary), Spec 10 seed (sub-page tiling) |

**This file belongs in the repo** — `docs/` alongside the measurement records — and should be updated in place. The full spec lived outside the tree for the whole program, which is why sessions could not see their own backlog and had to be handed tasks one at a time. That is L08-053 at the largest scale it has appeared here: not a claim in the wrong file, but the entire plan in the wrong system.

Completed phases are omitted. History, retracted claims and the ADR ledger stay in the full spec.

---

## Immediate

Ahead of Phase 6, in this order.

### CLAUDE.md ceiling backlog is stale

**Done (r90).** Corrected to the real four, and `check-file-ceiling.py` now compares the table's rows against the baseline by membership and fails either way — a sentence saying "believe the baseline" is the shape that drifted in the first place.

### Ribbon overflow menu — live defect (I-28)

**Done (r91).** Hosted in `AtPopoverHost`; `scripts/sitting/run.sh ribbonoverflow` is the regression. The sweep also deleted `overlay`'s backdrop mechanism, whose last requester this was. API strain reported, not absorbed: `route_key(Panel, Tab)` is `FocusNextControl` and nothing performs it — the `advance_focus_past|focus_next_node` register row now has two waiting consumers.

### Harness lies — done (r92)

`click_at` fails loudly when a click changes nothing anywhere; every scenario's clicks go through it. It found two silently-dead steps in the colour-picker scenario on its first run. Window geometry no longer leaks between runs. A reported finding was retracted with it — see below.

### Branch review — the findings in this branch's own code (r93)

Seven, all fixed. Four were the same shape — **a decision with no reachable
consumer** — which is the shape this branch kept finding in *other* people's code
and then reproduced:

| Finding | Fix |
| --- | --- |
| `present` grew a floored overlay **downward regardless of side**, putting an `Above` menu across the trigger that opened it. The no-overlap invariant was asserted against `place`, and every consumer reaches geometry through `present` | Grow away from the anchor, side-dependently. Forced the three constraints (floor / anchor / viewport) into a stated precedence — the viewport concedes, because a clipped overlay *looks* clipped where one over its own trigger looks correct. `!rect.is_inside(vp)` is now the detectable signal a real modal fallback needs |
| The zoom menu cleared its typed field in a `KeyAction::Dismiss` arm of its own `on_key`. **The host answers `Dismiss` itself and never forwards it**, so one Escape left the field open with a stale value owning the keyboard for the rest of the session | Reset in `on_dismiss` — the complete hook, which also covers the outside click and the anchor leaving. `KeyAction::dismissal_cause` now states the host-owned set once, and a test asserts it from the consumer's side |
| The calibrate dialog believed **any positive number** while `calibrated_css_ppi` refuses a ratio outside `0.5..=2.0`. The centimetre mistake its own prose warns about cleared the rejection notice and was then dropped by the caller: Apply did nothing, silently | `on_measured` returns `bool`. The dialog keeps the parse; believability stays with its one owner and the refusal is unavoidable rather than documented |
| `scale_resolve::resolve` applied the capability cap **before** `document_view` seeded the editor's canonical layout — and the cap reads page sizes. `provide_paginated_layout` is a no-op once the cache is filled, so the renderer laid the document out itself and the editor's layout was dropped: a second full layout per generation, and the ~20 MB font scan on open. Introduced by r80; nothing failed | Seeding moved inside `resolve`, between the mode and the cap. Asserted by `Arc::ptr_eq` — the only instrument that separates *reused* from *recomputed to look the same* |
| The capability search (up to **3040 `plan_residency` calls**) ran on every render, including every scroll frame, to re-derive a number that had not moved | Memoised on its three real inputs, key and value under one lock so they cannot desync. Counted, not timed |
| `loki-spreadsheet` and `loki-presentation` mounted `AtPopoverHost` without `use_provide_window_size`, so every menu placed against an **unbounded** viewport — never flips, never clamps | Both now feed the context from the `AtWindowSizeSensor` they already had |
| `anchor_scope::reposition` had a second idempotence guard comparing a pair `on_anchor_change` has already proved differs | Removed. A guard that cannot fire reads as the obligation being met there, so the next reader finds a decoy instead of the arm that enforces it |

**Sat on a screen** (`scripts/sitting/run.sh ribbonclearance`, new). At
HEIGHT=420 the menu clears its trigger (12 of 3136 px changed, against a control
crop inside the menu at 3136 — an instrument that could not report occlusion
would report none everywhere). At HEIGHT=150 the placement is pinned at the 88 px
floor and still leaves the 4 px gap; the pre-fix build at the same geometry puts
it six pixels lower, past the anchor's top edge.

**And the reading has a stated limit.** Six pixels is the whole discriminating
band this harness can produce: below HEIGHT≈145 the ribbon drops its control row
and the More button stops existing, so the case where the menu covers the whole
trigger is unreachable on X11. It needs `viewport.y > 0`, which only the Android
safe-area path produces. The plain-`AE` first draft of the measurement was also
wrong — a dismissible popover tints every pixel in the window, so it reported
2552 of 3136 "overpainted" on a trigger that was demonstrably clear. 2% fuzz
clears the tint and nothing else.

### The popover layer was rendering in serif (r94)

Reported from the app: new UI elements "showing up with the wrong font, like
they're unstyled". They were unstyled. `font-family` was declared
component-by-component — about twenty-five copies of one fact — and **nowhere on
the document**, so anything not inside such a component fell through to the CSS
initial value. `AtPopoverHost` mounts at the app root by construction, so its
content inherits from `body` and nothing else: **all six** popover content
producers (zoom menu, spelling menu, Recent ⋮ menu, Open tooltip, ribbon overflow
menu, the host itself) declare no family.

Fixed at the document root — `appthere_ui::ui_font_css()`, the shape
`focus_ring_css()` already established, injected by all three apps. Declaring it
at the twenty-sixth component would have left the twenty-seventh to find.

**Measured, both polarities, in one run of `run.sh zoom`:** the Home screen and
the editor are pixel-identical before and after (0 and 0 — everything there
already declared a family), and the zoom menu changes by 3759 px, from a visibly
serif face to Atkinson. `ribbonclearance` is also 0: the overflow menu's labels
come from `AtRibbonGroup`, which declares one, which is why that menu looked
right and hid the defect.

**Two things this did not settle.** The first probe reported *nothing* changed —
it photographed a scene whose every label already had a family, so a control
injecting `Cousine` at the root changed nothing either; a null result from an
instrument that cannot speak. And the `menu` scenario turned out to be broken by
leftover `recent.json` state — the same leak class as r92's `window.json`, now
cleared alongside it. The Recent ⋮ menu is therefore fixed by enumeration and
inheritance, not photographed.

### The cleanup pass (r95)

**91 declarations removed across 55 files.** The count in r94 said "~25" — that
was `appthere-ui` alone; tree-wide it was 86 naming the UI token plus five that
did not. All 86 were verified to bind to `FONT_FAMILY_UI` before anything was
touched, mechanically rather than by reading.

**Two of the five were not redundant — they were wrong.** The spreadsheet and
presentation editor roots declared `system-ui, sans-serif`, overriding the
bundled face for their entire editor surface. An inline declaration beats a
sheet, so those two would have kept rendering in the wrong font after r94. They
are removed, which is a behaviour change and the point of it. Four genuine
exceptions stay: monospace for macro source and the formula bar, and the
spreadsheet's italic-serif row marker.

**Verified as a cleanup should be: ten screen sittings, every one 0 px.** Home,
editor with ribbon and status bar, zoom menu open, keyboard walk, zoom applied,
Actual Size, and the overflow menu at a narrow window — all pixel-identical to
the pre-cleanup build under the same harness state. A first attempt at this
comparison showed 68 218 px on the Home screen and was *not* a regression: the
baseline predated the `recent.json` reset, so the two runs had different Recent
lists. Re-baselined by stashing the cleanup, rebuilding, and re-shooting.

**Locked by `scripts/check-ui-font.py`** (gate 15, and in CI): no component may
declare `font-family`; the four exceptions live in `ui-font-allowlist.txt` keyed
by *family* rather than line number, and an allowlist entry whose declaration
has gone also fails. A deleted duplicate comes back, and two of the 91 had
already drifted without review noticing — a `font-family` line looks like
diligence.

**Not established:** the two `system-ui` removals are in apps with no sitting
scenario. Loki Calc's Home screen was photographed and renders in Atkinson, but
its *editor* root — the line actually changed — was not reached; the `editor`
scenario's Tab counts are calibrated for loki-text and do not open a document
there.

Not fixed, and not this branch's: the macro trust/signature stack, `loki-layout`'s
squiggle clamp, and the gate-script bypasses.

---

## Phase 6 — Page styles (I-04)

**Re-scoped XL → M by S0.5.** ADR-0012 Decision 2 already shipped `Length<Emu>`, `PageStyle`, the N-column model, ODT master-page round-trip, DOCX section export and `mirror_margins`. **Read ADR-0012 and confirm against the tree before writing anything** — this phase's scope was wrong by a wide margin once already.

### Scope audit against the tree (r96) — it was wrong again, in both directions

The instruction above was followed, and it earned its place. **ADR-0012's own
Consequences section is stale**: it says the page family is "decided but not yet
built", while `PageStyle`, `derive_page_styles` and the read-only page inspector
all exist. And the task list below over-states what remains:

| Task | Spec says | Tree says |
| --- | --- | --- |
| T6.1 `style:page-usage` | remaining | **Was remaining — done, r96.** |
| T6.2 page-size catalogue | remaining | **Done, r98.** 28 papers, plus user-defined. See below. |
| T6.3 app-scoped defaults | remaining | **Done, r100.** See below. |
| T6.4 unit resolution | remaining | **Done, r99** for the page surfaces; the rest of the panel still shows pt. See below — and the r96 wording was loose: typed `Length<U>` units existed all along, what was missing was a *runtime* one. |
| T6.5 advisory DOCX part | remaining | **Done, r101.** See below. |
| T6.6 odd/even + first page | remaining | **Already built.** DOCX reader, writer, `settings.xml` assembly, model fields, and `flow_headers::assign_headers_footers` selecting the first/even/default variant. The spec's "mirrored margins are near-useless without it" was already satisfied. |
| T6.7 UI panel + manager | remaining | **Was half built — the editing half is done, r97.** See the correction below: the r96 row was wrong about rename. |
| T6.8 conformance | remaining | **Remaining** for page styles specifically; the ODF round-trip suites exist to extend. |

**T6.1's premise checked out.** `mirror_margins` really is wired end to end —
`w:mirrorMargins` → `DocumentSettings` → `LayoutOptions` → `mirrored_margins()`
swapping left/right on even pages, with a Loro round-trip and tests. What did
*not* exist was any ODF spelling of it.

### T6.5 — the advisory DOCX page-style part (done, r101)

`/word/lokiPageStyles.xml` carries page-style **names** and the section → name
mapping through a DOCX round trip. OOXML has no named page style — a section is
its `w:sectPr` and nothing else — so a rename made in the style panel survived
ODT export (which has `style:master-page`, wired in T6.1's neighbourhood) and was
lost the moment the document was saved as `.docx`. That asymmetry is what D-02
exists to close.

**"Advisory" is four claims, and each is enforced rather than asserted.**

1. *Ignored by Word.* A private part, private content type, private
   relationship type; nothing in `document.xml` refers to it. A consumer that
   does not know it exists reads the document it would have read anyway.
2. *Never affects geometry.* The reader writes `Section::page_style` and catalog
   display names and touches no `PageLayout` field. Asserted end-to-end by
   exporting the same document twice — once named, once stripped — and comparing
   every section's layout across the two round trips, which also catches the
   case where the geometry is wrong in both.
3. *Validated against the `sectPr` count.* The part declares how many sections it
   described; a document whose count has changed since gets no names rather than
   the wrong ones.
4. *Discarded on mismatch.* The **whole** part, not the offending entry — a
   partial mapping leaves some sections named and others not, which is harder to
   explain than no names at all.

**The declared count is written explicitly, not derived.** A count computed from
the same `<section>` list it is meant to validate would agree with itself no
matter what — the instrument would be reporting on its own input. Deriving it
instead of reading it kills the stale-part test.

**The stale-part test edits a real `.docx`.** It exports, opens the package,
rewrites `sectionCount="2"` to `"3"`, re-zips, and re-imports — which is what a
Word round trip that merged two sections would leave behind. A guard asserts the
fixture really did declare two sections first, so the test cannot pass by editing
nothing.

**Not `customXml/`**: that is a public OPC affordance with its own item/itemProps
pair and a datastore Word surfaces in its UI. This is not user data.

**One duplication removed on the way through.** `write/custom_props.rs` and
`write/comments.rs` each carried a private `escape` function and this part would
have been the third; there is now one `xml_util::escape_xml`.

**A bug worth recording, because the type system permitted it.**
`local_name(e)` where `e: &BytesStart` *compiles* — `BytesStart` derefs to
`[u8]`, so it silently matched against the element's entire raw bytes,
attributes included, and every well-formed part was rejected. The failing tests
were the ones asserting successful parses; had the tests only covered rejection,
this would have looked like a working validator.

Mutation-tested four ways: dropping the count check, letting the part write
geometry, applying the valid half of an inconsistent map, and deriving the count
from the listed sections — each kills a specific test.

**Not established:** no real Word has opened one of these files. The claim that
Word ignores the part rests on it being unreferenced from `document.xml` with a
private content type, which is how OPC is specified to behave, not on an
observation. Whether Word *preserves* the part through its own save is untested
and out of our hands — the count check exists precisely because it may not.
Nothing outside DOCX uses the map; ODT continues to carry names natively.

### T6.3 — app-scoped defaults (done, r100)

`loki_app_shell::document_defaults` stores page size, margins, measurement unit
and remembered custom sizes in `AppThere/document-defaults.json`, alongside the
display calibration and window geometry — the persistence pattern this crate
already had, reused rather than reinvented.

**Seed, never embed, is enforced by *where the call is*.** `load_document` has
four arms — blank, bundled template, imported file, real file path — and the
defaults are applied on the **first alone**. A template carries the geometry its
designer chose and an opened file its author's; re-seeding those would silently
reformat other people's documents to the reader's preferences, and would look
fine, because the result is still a well-formed document. The seeded document
carries ordinary geometry and no marker, so there is nothing to export and
nothing for a second machine to reinterpret.

**An unset field leaves the built-in answer alone.** `new_blank` has already
picked A4 or US Letter from the locale; replacing that with a hardcoded value
would make an empty settings file *worse* than no settings file.
`an_unrecorded_field_leaves_the_built_in_answer_untouched` covers the half-set
case too, which a single "have any settings" flag would get wrong.

**Three of the four fields had no writer, so controls came with the store.** The
seeding path *reads* `page_size`, `margins` and `measurement_unit`; only
`custom_sizes` was written, by the size field. A setting nothing can set is
indistinguishable from one that does not work — so the page form grew a "New
docs → Use as default / Reset" row and a unit picker. The Reset exists because
"Use as default" alone is a one-way door: a reader who set one by accident could
only replace it, never get back the behaviour they had.

**This is what gives T6.4's explicit rung a source.** `page_measurement_unit`
passed a literal `None` until now; it passes the stored setting, so the D-03
chain has all four rungs live rather than three.

**A settings file is not reactive state**, so writing one changes nothing on
screen. `StyleEditorSync` gained a `settings_generation` counter for the panel to
re-render on. The alternative — re-setting an unrelated signal to its own value —
works only because `Signal::set` does not compare, and reads at the call site as
a line that does nothing.

**Validation happens on load, once.** A hand-edited or future-version file is
untrusted input; an implausible page is dropped in favour of the built-in rather
than propagated into every new document, so no reader of the struct has to
re-check it. Margins are checked against a *different* range than pages: a zero
margin is legitimate (full bleed), a zero-width page is not.

Mutation-tested five ways: overwriting an unset field with a built-in, letting
margin seeding clobber header/footer/gutter, seeding only the first section,
remembering catalogued papers as custom sizes, and skipping the plausibility
check — each kills a specific test.

**Not established:** no screen sitting — the style panel still has no harness
scenario, so the two new rows have not been seen rendered. The store is written
and read only by `loki-text`; Calc and Slides link the same crate and ignore it.
Nothing reads the defaults at *file → new* time outside the editor route, and
there is no settings dialog — these controls live in the page-style panel because
that is where the geometry they describe already is, not because that is where a
preference belongs. D-07's list is complete (size, margins, unit, custom sizes);
a general preferences surface is not part of it.

### T6.4 — measurement units (done for the page surfaces, r99)

`loki_primitives::units::MeasurementUnit` — mm, cm, in, pt, pica — with D-03's
chain, formatting, and parsing.

**The r96 row was loosely worded.** It said "no measurement-unit type anywhere in
the tree", and the *typed* units (`Length<Mm>`, `Length<Inch>`, the whole
`UnitConversion` table) were there all along. What was missing is a **runtime**
unit: the user's choice of what to read and type in. Compile-time unit safety and
a display preference are different things, and the row conflated them.

**Precedence is structural, not documented.** `effective_measurement_unit` takes
the explicit setting as its argument and the environment-derived answer is
private, so there is no way to ask the environment while forgetting the user —
rule 5 applied to a four-rung chain. The chain itself is a pure function over
already-read values, so all four rungs are tested without touching the process
environment.

**The two locale tables stay apart.** `default_page_size_for_locale` reads the
same locale variables to pick A4 vs US Letter, and its region list is
deliberately *not* this one: Mexico, Canada and the Philippines take US Letter
paper and are metric. `mexico_uses_letter_paper_but_metric_units` asserts the
measurement table has not adopted the paper one's regions — a merge that looks
like tidying and gets one of the two answers wrong for all three countries.

**Display and entry only.** Nothing stored changes. The seed-then-read-back path
is asserted for **every paper × every unit**, because the panel seeds its custom
fields with `format_bare` and reads them with `parse`: if those two disagreed,
opening the panel and pressing Set without typing would resize the page, and only
in units the developer does not run in. That test is what fixes the decimal
places — coarsening mm/pt to 0 decimals kills it.

**Two rules that had to be decided rather than inherited.** The range check on a
custom size is applied *after* conversion, because 36 pt … 14400 pt is a property
of the page rather than of the number typed — checking the raw number would
reject every millimetre entry (A4 is 210 mm, under a floor of 36) and wave
through absurd inch ones. And margin equality is decided in points *before*
rounding, so whether the inspector shows one value or four does not depend on the
user's unit.

Mutation-tested six ways: swapping the OS and locale rungs, demoting the explicit
setting, adopting the paper-size regions, coarsening the decimals, ignoring a
typed suffix, and checking the range before conversion — each kills a specific
test.

**Not established, and deliberately bounded:** only the **page** surfaces follow
the unit — the page inspector's size and margins rows, and the custom-size entry.
The paragraph, list and character inspectors still print `pt` (`style_inspector`,
`style_list_inspector`, `style_char_inspector`). Font size is *correct* to leave
in points, as LibreOffice and Word both do; paragraph spacing, indents and tab
stops are **not**, and are a follow-up. There is also still no UI to *choose* a
unit and nowhere to persist one — `effective_measurement_unit(None)` is what
every caller passes today, so in practice the environment decides. The settings
store is T6.3, and the signature is already the shape that will take it.

### T6.2 — the paper catalogue (done, r98)

`loki_doc_model::layout::paper_catalog` holds the 28 papers T6.2 listed — ISO
A0–A6, ISO B4–B6, JIS B4–B6, C5/C6/DL, US Letter, Legal, Tabloid, Executive,
Statement, Folio, Quarto, #10, Monarch, and the three index cards — plus a
user-defined width × height entry in the page-style panel.

**A paper name is derived, never stored.** Neither format carries one: ODF writes
`fo:page-width`/`fo:page-height`, OOXML writes `w:pgSz/@w:w` and `@w:h`. (OOXML's
`@w:code` is a Windows `DEVMODE` printer paper code, which Loki neither reads nor
writes.) So `PageSize` gains **no name field** — one fact, the dimensions, and
one derivation from it. A size the catalogue cannot name is a user-defined size,
which is a normal state rather than an error.

**Three copies of one predicate collapsed into it.** "Is this page that paper?"
— orientation-independent, ±1 pt — was written out three times, in the style
inspector, the page form's active-preset check, and the Layout ribbon, each
alongside its own dimension literals. All three now call `Paper::matches`, and
`PageSize::a4()`/`letter()` are defined *by* the catalogue rows that name them,
so the dimensions a page is set to and the dimensions it is recognised by cannot
drift apart. The ribbon's `PAGE_SIZE_PRESETS` in particular held a second copy of
595.28 × 841.89.

**The two-size cap was structural, not a UI limit.** `PagePreset` had one variant
per paper (`SizeA4`, `SizeLetter`), each carrying its own literals and its own
comparison — so a third size meant a third variant. One `Size(&'static Paper)`
variant makes the whole catalogue reachable through the applier that already
existed.

**The tolerance is now defended by an inversion.** `MATCH_TOLERANCE_PT` is 1 pt,
and matching requires *both* axes within it — so what bounds the constant is each
pair's **better**-separated axis, minimised over the catalogue, which is 12.47 pt
(A6 vs Index card 4×6). Several pairs share one edge exactly (Folio and Legal are
both 612 pt wide) and are still never confusable, so the closest single edge is
the wrong number to reason from. `paper_entries_are_mutually_distinguishable`
computes the right one and asserts the tolerance stays under it: rather than
checking each paper matches itself — which passes for any tolerance, however
wide — it checks that no paper matches another. Widening the tolerance to 20 pt,
or changing `matches` from AND to OR, each kill it.

**Custom sizes are rejected, not clamped**, outside 36 pt … 14400 pt, and the Set
button is withheld while the entry is unusable rather than shown inert.

**Not established:** no screen sitting, again — the style panel still has no
harness scenario, so the 28-button grid has not been seen rendered, and its
wrapping behaviour in the Compact posture is unverified. The catalogue's display
names are `&'static str` constants, not Fluent keys: correct for the proper nouns
(A4, DL, Tabloid), wrong for "US Letter", "Legal", "Executive", "Statement" and
the index cards — `TODO(paper-i18n)`. A searchable dropdown (T6.7's "catalogue
with search") still wants the panel hosted in `AtPopoverHost`.

### T6.7 — the manager verbs (done, r97), and a correction to the r96 row

**The r96 audit row above was wrong**, in the direction it warned about. It said
"creating, renaming and applying a page style is not there". *Renaming* was
there — `page_rename.rs`, `rename_page_style`, catalog key + every section
reference — and so was geometry editing through preset buttons. The row was
written from the *inspector's* module docs, which say "read-only" and are
accurate about `style_page_inspector.rs` while `page_form.rs` sat beside it
doing the writing. An audit that reads the doc comment of the file it happens to
open reproduces exactly the error it was called in to catch.

What was genuinely missing were the two verbs that make the family a *manager*:

- **`create_page_style`** — a catalogued style seeded from the selected style's
  geometry, named with the next free `PageStyleN`.
- **`set_section_page_style`** — put a style on the section the caret is in, and
  **give that section the style's geometry**, so applying changes the pages
  rather than the label they carry.

They land as one unit because either alone is inert: a created style no section
references paints nothing and exports nothing, and there was no way to reach a
section's page-style assignment at all.

**The column cap is gone.** `ColumnCountDelta` steps to `MAX_COLUMNS` (12) and
`ToggleSeparator` exposes `SectionColumns::separator` — modelled, written by both
exporters, painted by the layout engine, and settable from no UI in the suite
until now. Both route through `apply_preset`, so stepping and the 1/2/3 presets
cannot drift over what "3 columns" means.

#### Three defects this turned up, none visible before the verbs existed

1. **The catalog's geometry copy was write-only.** `PageStyle.layout` was read
   by nothing in production — `set_page_style_geometry` wrote sections only.
   Harmless while unread; the moment `set_section_page_style` could seed a
   section from it, a stale entry became a way to apply geometry no page had
   shown. The mutation now writes both copies in one call.

2. **`apply_preset`'s column-width logic was unreachable.** It carefully
   preserved per-column widths when the count still matched and dropped them
   otherwise — and `set_page_style_geometry` never wrote the widths key at all,
   so a width list from a different count survived every preset. The layout
   engine's `widths.len() == count` guard meant this degraded quietly instead of
   breaking, which is why it lasted.

3. **The geometry tests targeted by a mechanism production does not use.**
   `page_style_geometry.rs` derived its section indices from
   `section_page_style_ids` (layout-equality grouping) while the panel targets
   the stored `section.page_style` reference. The two agree on that fixture and
   diverge the moment two names share a geometry. The fixture now runs
   `assign_page_styles` and targets by name, like the panel.

**`derive_page_styles` / `section_page_style_ids` have no production callers at
all** — only tests and their own re-export — despite doc comments calling them
"the export inverse". ODT export uses `resolve_page_style_names`, which honours
the stored reference. Left in place and **not** deleted this pass; flagged here
rather than silently kept.

**Mutation-tested, seven ways** — four in the model, three in the panel. Each
of: dropping the catalog write, keeping stale widths, setting the reference
without the geometry, preferring the catalog over the live section, listing only
applied styles, reading the flat block index as a section index, and removing
the column clamp — kills a specific test.

**Two of the seven passed on the first attempt and shouldn't have**, both from
the same failure: a control that silenced its own subject.

- The live-section-outranks-catalog test couldn't discriminate because
  `set_page_style_geometry` keeps the two copies equal, so *no* setup using it
  can tell them apart. The discriminating case had to come from the Layout
  ribbon's `set_document_*` mutations, which write sections and never touch the
  catalog.
- The panel's section-first read passed under mutation because the test moved
  its sections to **Letter** — and `PageSize::default()` *is* Letter, so the
  "changed" sections held exactly what the stale catalog held. Moving the
  fixture to A4 fixed it. Both tests now carry a guard asserting the two sources
  genuinely differ *before* reading one, so a future change that re-equalises
  them fails loudly instead of passing quietly.

**Not established:** no screen sitting — the style panel has no harness
scenario, so this is verified by unit and model tests only, and nothing here has
been seen on a screen. Duplicate, delete, set-default, catalogue search,
unit-aware margin fields and live preview (the rest of T6.7's line) are **not**
built.

Two gaps are marked in-code rather than left to be rediscovered:
`TODO(page-styles-export)` in `odt/write/page_styles.rs` — a catalogued page
style **no section references** is not written to ODT, because that walk is over
sections (it does survive the Loro CRDT, so it is not lost in-session, and ODF
permits an unreferenced `style:master-page`); and `TODO(page-panel-touch)` in
`page_form.rs` — the stepper's `−`/`+` are the panel's smallest hit targets and
are not covered by the Compact posture's `touch_min_css()`, relying on the
ambient font scale instead.

### T6.1 — `style:page-usage` (done, r96)

`PageUsage {All, Mirrored, Left, Right}` on `PageLayout`, with the ODF codec in
`loki-doc-model/src/layout/page_usage.rs`. Read and written by the ODT
reader/writer, carried through the Loro bridge, and — the half that makes it a
feature rather than a field — **read by the paginator**, so an ODT that mirrors
now alternates its margins.

**The formats disagree about where the property lives, and the model takes the
richer shape.** ODF states it per page layout; OOXML has only the document-wide
`w:mirrorMargins`. So the DOCX importer stamps the flag onto every section, the
DOCX writer asks `Document::mirrors_margins()` (a union of both origins, because
an ODT-sourced document has no `settings` at all and the setting-only read
exported it as single-sided), and the collapse happens at the one boundary where
the format forces it.

Mutation-tested both ways: reverting the paginator to the settings-only read
kills the mirroring test; making the writer emit the attribute unconditionally
kills the default-bytes test.

| Task | Work |
| --- | --- |
| T6.1 | `style:page-usage` (`all`/`left`/`right`/`mirrored`) on the ODF path, wired to existing `mirror_margins` |
| T6.2 | Page-size catalogue: ISO A0–A6, ISO B4–B6, JIS B4–B6, C5/C6/DL, US Letter, Legal, Tabloid, Executive, Statement, Folio, Quarto, #10, Monarch, index cards, plus user-defined |
| T6.3 | App-scoped defaults per D-07: size, margins, unit, saved custom sizes seed new documents but never embed. Styles stay document-scoped |
| T6.4 | Unit resolution per D-03: OS measurement setting → locale region → metric; explicit user setting overrides. Display and entry only, model stays EMU |
| T6.5 | Advisory DOCX custom part per D-02: names and section mapping, ignored by Word, recovered on our reopen, never affects geometry, validated against `sectPr` count, discarded on mismatch |
| T6.6 | Odd/even and first-page header/footer variation (`w:titlePg`, `w:evenAndOddHeaders`) — mirrored margins are near-useless without it |
| T6.7 | UI: page style panel (catalogue with search, orientation, unit-aware margin fields with live preview, columns, gutter) and manager (create/rename/duplicate/delete/apply/set default). **Replace the three column preset buttons — that is the entire 1–3 column limit.** Preserve existing margin and size presets as shortcuts |
| T6.8 | Conformance: mirrored margins, custom size, N-column with separator, multiple page styles per document, DOCX → ODF → DOCX round trip |

**Format constraints (§3.4).** DOCX has no page-style concept: geometry lives on `w:sectPr`, mirroring is the document-wide `w:mirrorMargins` flag in `settings.xml`, not per-section. LibreOffice itself drops names on export. `w:cols` supports up to 45 columns.

**Carried into T6.1 from Phase 5.** Per-section page area makes the servable-zoom limit a per-section property — scrolling Letter → A2 changes it with no user action, so neither of T5.4's gating moments applies. Two shapes sketched, neither chosen: whole-document minimum at load (never surprises, penalises one A2 insert in a hundred Letter pages) versus per-section non-retroactive (preserves don't-disrupt, but the maximum moves as you scroll, and scrolling into A2 at 300% enters `ceiling_exceeded` — the OOM branch — with nothing gating it). That asymmetry argues for whole-document minimum or a third gating moment.

**Acceptance.** LibreOffice opens our ODF with names and mirroring intact; Word opens our DOCX with correct geometry; round-trip green; existing documents migrate without visual change.

---

## Phase 7 — Mobile and reflow (I-11, I-14, I-15)

| Task | Work |
| --- | --- |
| **T7.0** | **Probe P1 first.** Build a scratch nested scroll container; check it consumes wheel and drag within its bounds and bubbles the remainder. **Test input routing, not layout** — `blitz-dom` models the geometry (`scroll_node_by_collect_inner`), so the plausible failure is that it renders right and routes wrong. Gates T7.3 |
| T7.1 | Status bar priority order, dropping items by **measured width**. Minimum retention: page indicator and zoom, rest behind a `Popover`. The ribbon overflow menu is this task's future consumer |
| T7.2 | Reflow typography decoupled from page metrics. Measure is a user setting defaulting to ~72–80 characters at body size, resolved against live font metrics (D-05) |
| T7.3 | Oversized elements shrink to fit the content column, aspect preserved, per-element expand into its own horizontal scroll container. **The document never scrolls horizontally** |
| T7.4 | If P1 says nested containers do not route, fall back to a modal full-screen viewer and record the deviation. Never ship a version where the document scrolls sideways |

**Acceptance.** No horizontal document scroll at any width with a 200%-width table present; status bar legible at 320 px; readable at default zoom on a phone without pinching.

---

## Outstanding readings — Kevin's, none blocking each other

| Item | Procedure |
| --- | --- |
| **Drift check** | Five minutes, no fixture. Open the spell menu **fresh at several editor scroll offsets** — not scroll with it open. Three readings are in `editor_spell_place`'s docs: constant offset = coordinate space; growing offset = origin error; correct-then-drifts-on-scroll = the absent driver, expected. With `spell_menu_anchor`'s sweep green, a drift points at Blitz's dispatch, not our arithmetic |
| **R5b** | `LOKI_TEXTURE_CEILING_MB=300`, plain Letter, no ballast, A3 optional. Look for a `raster_permille` line with **no** full-scale follow-up |
| **I-06** | The document that reported the squiggle. Read its `w:lineRule` and body font size against the 8×4 sweep table — 11pt/12pt default is a 0.000 cell and confirms nothing; 14pt default is the worst cell |
| **T5.5** | Actual Size within 2% — needs a physical display and a ruler |
| **Save on a titled document** | NOT a defect and NOT established. The reported "Save never clears the dirty dot" was wrong twice over: the run that produced it never reached Save, and the behaviour is correct — the only document the harness can open is untitled, which routes to Save As and is dirty by definition. Settling the titled case needs a document with a path, which needs a file picker (unavailable headless) or a path argument to the binary (loki-text takes none) |

---

## Open issues

| ID | Item |
| --- | --- |
| I-18 | Layout-memory tail → **Spec 09**, parked at its boundary after S9-1/S9-2 took 41% off residency |
| I-22 | Sub-page tiling → **Spec 10 seed**. Three independent evidence lines; would delete most of Phase 2's budget machinery |
| I-25 | Cross-platform available-memory divisor comparability. Probes ship; `available_permille_of_total` collects the data |
| I-26 | `reduced_motion` unwired at both ends while presenting as a capability. Consumers: T1.1's driver, Phase 5's zoom animation |
| I-27 | ADR-0013: seven `if cond { plain_function(..) }` panels; `AtPanelHost` has **zero mount sites**. First adoption changes the cost of the other six |
| I-28 | Ribbon overflow menu (above) |

---

## Register — 8 `awaiting` rows

One live defect (I-28). One degraded but marked: Tab out of a popover lands on the trigger rather than past it — `set_focus` is a bool, so `AdvanceFocusPastAnchor` cannot be expressed; costs one extra Tab, logged not silent.

Six gated on things that do not exist yet: frame-completion timer (current counter is a submission lower bound), platform accessibility text scale, a modal fallback to produce `DismissCause::PresentationChanged`, `:focus-visible` engine support, a `ThemeColor` producer, `AtPanelHost`.

The modal-fallback row acquired a **trigger condition** in r93: `present` now leaves the viewport rather than crossing the anchor when the floor cannot be honoured on either side, so `!placement.rect.is_inside(viewport)` marks exactly the geometry where the anchored form has genuinely failed. Under the previous rule that case was indistinguishable from a good placement, so nothing could have escalated out of it.

**Phase 5 blocked tail:** theme swatches (no importer reads a theme part), alpha (no consumer carries it), eyedropper (screen-capture permission), `TODO(t5.6-effective-anchor)` — unreachable because the page-fit rule flips to reflow before the capability cap engages.

---

## Cross-cutting debt

Four files over the 300-line ceiling. Three vulnerable transitive `quick-xml` copies (0.30/0.38/0.39 alongside our patched 0.41), all upstream-gated as documented. ~40 distinct TODO topics tree-wide; Phase 5's six audited, ~34 older and unaudited.

---

## Standing disciplines

Full ledger in the spec; the digest lives in `CLAUDE.md` and loads by default. The ones this phase will need:

- **Verify spec claims against the tree** (L08-015). This spec has been wrong about its own code repeatedly — three r1 capability claims, most of Phase 6 already built, T1.5 and T3.2 needing no code, `set_zoom` not the single clamp site.
- **Correctness is necessary; placement decides whether it takes effect** (L08-053). When a rule is not taking effect, first hypothesis is placement, not phrasing.
- **Predict before implementing** (L9-013), naming the governing metric first.
- **Prefer counters to clocks** (L08-038). Four timing attributions retracted; no counter has been wrong.
- **Make the wrong thing unavailable, not discouraged** (L08-043).
- **Verify the fixture produces the precondition** (L08-044) — and that the precondition is false where the bug cannot manifest.
- **Mutation-test extracted arithmetic** (L08-041). A surviving mutation means either no test detected it or the code does nothing.
- House standards: 300-line ceiling (split, don't baseline), `#![forbid(unsafe_code)]`, `thiserror`, no `unwrap`/`expect` in library code, SPDX headers, Rust 2024, `fl!()` for every user-visible string.
