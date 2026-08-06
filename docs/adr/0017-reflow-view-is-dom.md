<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0017 — The reflow view is DOM, not canvas

| Field | Value |
| --- | --- |
| Status | **Accepted** — 2026-08-03. Direction agreed; view built behind a flag (§5). **Line breaks match the canvas path on every measured fixture and width** (§5.3–§5.6); virtualisation measured and does not bite (§5.7); lists agree up to one pixel of column, with the remaining difference attributed to a canvas-path defect (§5.9) |
| Drivers | Spec 08 T7.0 (probe P1), T7.2, T7.3, T7.4 |
| Affects | `loki-renderer` (reflow path), `loki-layout` (reflow mode), `loki-text` editor route |
| Makes moot | The ambient reading-measure cap added for T7.2 (`loki_renderer::measure`) |

---

## 1. Context

Two views of a document exist. The **paginated** view is a fidelity view of a
page with a real physical width, painted into wgpu/vello tiles — that is the
right shape for it and this ADR does not touch it. The **reflow** view is a
reading view with no physical width, and it is painted the same way, into a
canvas.

Three of Phase 7's tasks each stopped at a question that turns out to be this
one:

- **T7.3** wants an oversized element to expand into *its own horizontal scroll
  container*, so the document never scrolls sideways. There is no per-element
  box in a canvas to make scrollable.
- **T7.2** resolves the reading measure against live font metrics and has to
  publish it as process-wide ambient state, because the width is consumed during
  painting and hit-testing where the font lock cannot be taken.
- **T7.0** ran probe P1 to gate T7.3 and measured **DOM** nested scroll
  containers. They route correctly — consume within their bounds, bubble the
  remainder, and a horizontal-only inner does not swallow a vertical gesture.
  That reading is real and reproducible, and it applies to a path the canvas
  reflow view does not use.

P1 was run to gate T7.3 and answered a question about a different rendering
path. That is the finding this ADR acts on.

## 2. Decision

**The reflow view renders as DOM.** The paginated view stays on the canvas.

## 3. Consequences

### 3.1 What this buys

**T7.3 becomes small.** An oversized element becomes a DOM element with
`overflow-x: auto`, and P1's measurement is then evidence about the code that
actually runs. Nested routing, clamping and bubbling are Blitz's, already
verified, and not re-implemented.

**T7.2 becomes moot.** The measure is `max-width` on the reading column, in CSS,
resolved by the same layout that paints. The ambient cap
(`loki_renderer::measure`) exists only because the width had to cross from the
font-holding layout pass to the painting pass through a static; with one pass
there is no crossing. It should be **deleted, not left dormant** — a mechanism
kept "in case" is the shape ADR-0016 was written about.

The `loki_layout::measure` resolver is **not** made moot: resolving a character
count against live font metrics is still needed, and the answer becomes a CSS
length instead of a static.

**The single-source hazard disappears.** `reflow_layout_content_width_pt` is
called from four sites across two crates today precisely because paint,
hit-testing and navigation each rebuild the same layout. In DOM there is one
layout and the browser owns hit-testing.

### 3.2 What this costs, and what is not yet known

**Text fidelity was the risk. It was measured on 2026-08-03 and the two paths
agree.**

The canvas path shapes with Parley against `loki-layout`'s own resolution of the
document's character properties; Blitz shapes with Parley too, but through
Stylo's CSS cascade from styles we emit. Whether a paragraph sets identically
through both was the question that decided whether this ADR survives contact.

*Procedure.* One paragraph, Liberation Sans 12 pt / 16 px, laid out through
`loki_layout::layout_document` in `LayoutMode::Reflow` across 180–620 px in 2 px
steps, counting distinct glyph-run baselines; then the same text rendered in DOM
by `appthere-ui/examples/linebreak_probe.rs` at each **transition** width ± 2 px,
with an explicit `line-height` so the count reads off the block height.

Transition widths are the discriminating points: at a coarse width the break is
unambiguous and agreement proves little, whereas where the count changes the
decision is marginal, and marginal is where two shapers diverge first.

*Result — all seven transitions matched, each pinned to a 2 px window:*

| lines | layout side | DOM side |
| --- | --- | --- |
| 10 → 9 | 182 px | 180 = 10, 182 = 9 |
| 9 → 8 | 192 px | 190 = 9, 192 = 8 |
| 8 → 7 | 218 px | 216 = 8, 218 = 7 |
| 7 → 6 | 272 px | 270 = 7, 272 = 6 |
| 6 → 5 | 306 px | 304 = 6, 306 = 5 |
| 5 → 4 | 370 px | 368 = 5, 370 = 4 |
| 4 → 3 | 520 px | 518 = 4, 520 = 3 |

*Why they agree, and what that does not cover.* Both paths shape with **the same
engine** — Parley — so the test is not really about shaping; it is about whether
our property resolution and the CSS we emit perturb it. For plain LTR Latin text
in one face at one size, they do not.

**Not established**, and each is a way the agreement could still fail: mixed
style runs within a paragraph (bold/italic spans, per-run family or size),
letter-spacing and word-spacing, justified text, tab stops, hyphenation,
non-Latin scripts, and font fallback where the requested face is absent. The
probe extends to each by changing its text and styles; the layout sweep extends
the same way. **The ADR is supported, not proven**, and the next extension worth
running is mixed style runs, because a document is rarely one uniform run.

*Update 2026-08-04:* named styles, indents, alignment and **font substitution**
have since been swept on a real styled document — the screenplay template — and
also agree at every width; see §5.3. Mixed runs *within* a paragraph remain the
open case.

**Spell squiggles, selection geometry, the caret and revision marks** are all
painted from `PositionedItem`s today. Each needs a DOM equivalent or an overlay.
None is obviously hard; none is free.

**Virtualisation.** The canvas path presents the reflow layout as fixed-height
virtual tiles (`render_layout`) and only rasterises what is on screen. A DOM
reflow view of a long document builds a large tree instead. Blitz's cost profile
there is unmeasured. *What would settle it:* the `load_bench` open-latency
benchmark in `loki-acid`, run against a long document on both paths.

*Measured 2026-08-05 — **it does not bite**; see §5.7.* The DOM path reaches
first paint no later than the canvas path finishes `layout_document` for the same
document, at every length up to 8000 paragraphs, and both are linear at about the
same slope. What the canvas path virtualises is **rasterisation**, and layout is
the larger term. The cost that does grow is memory: ≈ 38 KB per paragraph.

**Two rendering paths remain**, since paginated stays on the canvas. Anything
shared — spell state, revision display, the measure — is consumed by both, so
this trades one hazard (ambient state crossing passes) for another (two paths
that must agree about the document).

### 3.3 Sequencing

1. ~~The line-break comparison in §3.2.~~ **Done 2026-08-03 — the paths agree.**
   Extend it to mixed style runs before relying on it further.
2. ~~Build the DOM reflow view behind the existing view-mode switch, so both
   paths are live and comparable.~~ **Done 2026-08-03 — see §5.**
3. ~~Move T7.3's per-element scroller onto it.~~ **Done 2026-08-05.**
4. Delete `loki_renderer::measure` **in the same change** that replaces it —
   see §3.1.

## 4. Status of the work this supersedes

`loki_renderer::measure` and its tests stay in the tree for now: the canvas
reflow view is what ships today, and the cap is the only thing that would make
its column follow the measure. It is marked here rather than in a comment,
because a comment on a module that is about to be deleted is the marking that
decays first. **It is parked, not forgotten** — this section is its reference
count.

---

## 5. Step 2 — the DOM reflow view exists (2026-08-03)

`loki-text/src/routes/editor/dom_reflow/`, reached with `LOKI_REFLOW_DOM=1` and
the view mode set to Reflow. `render_canvas_area` returns to it before any
canvas wiring, so the two paths are mutually exclusive and the existing switch
selects between them. `scripts/sitting/run.sh domreflow` photographs the same
document through both.

**It renders**, and the reading measure is what this ADR said it would be: a
`max-width` on the column, in CSS, applied by the same layout that paints it —
no ambient static, no ordering hazard between the pass that resolves it and the
pass that reads it.

### 5.1 Catalog resolution (done, same day)

The first cut applied only **direct** properties, so a screenplay rendered
proportional and left-aligned against the canvas path's monospaced, centred
dialogue. Fixed by resolving through the catalog — and specifically by resolving
through **`loki_layout`'s own resolver**, `resolve_para_props` and
`flatten_paragraph_with_base`, which is what the canvas path shapes with.

That choice is the point. A second resolver reading the same catalog would be a
second copy of the cascade, and the cascade is exactly the thing whose second
copy drifts. Reusing the same functions means the two paths cannot disagree
about what a style *means*; any remaining difference is about rendering, which
is what this comparison is for.

It also simplified the CSS. Resolved properties are definite values, so each is
written out rather than left to inherit — nothing is delegated to Stylo's
cascade, so Stylo's cascade does not have to agree with ours.

**Measured after:** alignment, centring, the right-aligned `CUT TO:`, indents and
paragraph spacing all now match the canvas path.

### 5.1a Font substitution, and the last two block kinds (done, same day)

Two further differences, both the same shape — a `loki-layout` service the DOM
path was bypassing rather than a mapping error.

**Substitution.** The canvas path rendered the screenplay monospaced and the DOM
path proportional, with "1 font substituted" in the status bar on both.
`loki-layout` resolves a missing family to a metric-compatible face through
`FontResources::resolve_font_name`; the DOM path was emitting the family the
document *asked* for and letting Blitz fall back by its own policy. Two
substitution policies for one missing font.

Every requested family is now routed through that same resolver. Resolved once
per render into a map rather than per run: `resolve_font_name` takes
`&mut FontResources`, and holding that lock across the render would put a
shaping mutex in the middle of the UI thread's tree build.

**Headings and bare paragraphs.** `Block::Heading` and `Block::Para` are not
`StyledPara`, so they never reached the resolver — the heading stayed
proportional after the body went monospaced. The canvas path converts them with
`synthesize_heading_para` / `synthesize_plain_para` before resolving; those are
now `pub` and the DOM path calls the same two. Re-deriving them would have been
a second statement of which style a heading level names.

The family collector walks the synthesised forms too. Collecting only
`StyledPara` would have left every heading emitting its requested family
unsubstituted — the one run still rendered by a policy that is not ours.

**Measured after:** the heading, the body, the centred dialogue and the
right-aligned `CUT TO:` all match, and **every line breaks at the same word on
both paths**.

### 5.2 What it deliberately does not do

Read-only: no caret, selection, hit-testing, spell squiggles or revision marks.
Tables, lists and images render a **visible placeholder** rather than nothing —
a silently dropped table would make the two paths look closer than they are,
which is the one failure mode a comparison instrument must not have.

### 5.3 The styled document's line breaks, measured (2026-08-04)

§5.1a's "every line breaks at the same word on both paths" was **by inspection**.
Swept, it still holds — and the sweep is worth having, because it is the first
comparison that exercises named styles and font substitution.

*Procedure.* `cargo run -p loki-text --example styled_linebreak_sweep` lays the
**screenplay template** out through `layout_document` in `LayoutMode::Reflow`
across 260–800 px in 1 px steps, counting distinct glyph-run baselines, and
prints the widths where the count changes. `scripts/sitting/run.sh
styledlinebreak` then renders the same document through **the real DOM view**
(`dom_reflow::document_view`, not a second emitter) at each of those transitions
and the pixel below it, all in one row, and `scripts/sitting/linebreak_bands.py`
reads each column's height. With `line-height` pinned, a band is
`constant + lines × 24 px`; the constant is calibrated **once** and then has to
reproduce every other width.

*Result — all nine transitions matched, each pinned to a 1 px window*, with one
constant (184 px) across all eighteen columns. The document resolved
`"Courier New" → "Cousine"`, so this run does cover substitution.

| lines | layout side | DOM side |
| --- | --- | --- |
| 18 → 16 | 269 px | 268 = 18, 269 = 16 |
| 16 → 15 | 279 px | 278 = 16, 279 = 15 |
| 15 → 14 | 298 px | 297 = 15, 298 = 14 |
| 14 → 13 | 346 px | 345 = 14, 346 = 13 |
| 13 → 12 | 356 px | 355 = 13, 356 = 12 |
| 12 → 11 | 404 px | 403 = 12, 404 = 11 |
| 11 → 10 | 423 px | 422 = 11, 423 = 10 |
| 10 → 9 | 529 px | 528 = 10, 529 = 9 |
| 9 → 8 | 586 px | 585 = 9, 586 = 8 |

*The instrument discriminates.* An agreement is only worth the failure it could
have shown, so the DOM half was run once with its font registration removed:
the counts then disagreed at **14 of the 18 widths** and not one transition
landed in the same window (268 and 269 both read 14 against the layout side's 18
and 16; 528 and 529 both read 8 against 10 and 9). No constant fits that run.

*And it caught something.* That failing configuration was not synthetic — it was
the probe's first version, and it failed because `resolve_font_name` answers with
a family from **`loki-layout`'s** font collection while Blitz has its own. The
app is fine: `main.rs` registers `loki_fonts::ui_font_blobs()`, which is the same
bundled set the substitution draws from. But the coupling is real and undeclared,
and it does not extend to a face `FontResources::new` finds in the
executable-relative `assets/fonts/` directory, which Blitz never scans —
`TODO(dom-reflow-fonts)`, recorded on `dom_reflow::resolve_families`.

**Still not established.** The screenplay is uniformly styled *per paragraph*: it
has named styles, indents, alignment and a substituted family, but no **mixed
runs within a paragraph** — no bold or italic span, no per-run family or size
change. §3.2's open item is therefore still open, and it is the next extension:
both halves take it by changing the document, not the harness. Also untouched:
justified text, tab stops, hyphenation, letter/word spacing and non-Latin
scripts.

*Read this section together with §5.4*, which ran that extension and found the
paths **do not** agree — and which explains why this one's agreement is weaker
evidence than it reads as: the screenplay is monospaced.

### 5.4 Mixed runs within a paragraph — **they do not agree** (2026-08-04)

> **Superseded in part by §5.5**, which located the cause — CSS whitespace
> collapsing eating the space *across* a run boundary — and fixed it. Read this
> section as the measurement that led there; its "unexplained residual" is now
> explained, and much smaller.

The extension §3.2 kept naming, run on a fixture built for it
(`loki-text/examples/common/fixture.rs`, `LB_FIXTURE=mixed:<case>`): the same
sentence six times, base style Arial 12 pt, with one middle run differing by
exactly one property — weight, italic, size, family, a named **character**
style, or letter-spacing. Both halves read the same fixture, so neither can be
laying out a different document.

**Result: every case disagrees**, at 2–5 of its 12 widths. Measured transition
positions, layout side against DOM side: `family` 6→5 at **435 vs 431 px**,
`size` 5→4 at **547 vs 543 px**, `weight` 8→7 at **289 vs ~300 px**. So a
transition moves by roughly 1–10 px, in both directions, and a paragraph near a
break flips a line. This is a **fidelity difference between the two views**, and
it is now on the record rather than assumed away.

#### What the controls established, and what they did not

Two controls, run before attributing anything to *mixedness*:

* **`mixed:plain`** — the same text with the middle run present but carrying
  **no** property change. It disagreed at 4 of 12 widths. So the disagreement did
  not need a property change at all.
* **`mixed:onerun`** — the same *characters* as one inline, no run boundary. It
  agreed at **all 12**.

The boundary was the variable, not the property. A `<span>` boundary is not free:
Blitz measures inline boxes item by item, and the canvas path shapes the
paragraph as one styled string.

#### First response: `content::coalesce` — which turned out to be an economy

Adjacent resolved spans whose emitted CSS is identical are now joined, so a run
split with no formatting meaning stops reaching the renderer. That made
`mixed:plain` agree — but §5.5 then found the real cause, and with *that* fixed
`mixed:plain` agrees **with coalescing switched off** too. So this is a node-count
economy, not a correctness fix, and it is documented as one. Recorded rather than
quietly re-labelled: the reasoning that produced it (a boundary must be costly,
because removing boundaries helped) is exactly the reasoning a coincidence
survives.

#### Not established, and what would settle it

*Answered by §5.5* — the answer was the third thing this section did not think
to list.

*What would settle it:* compare **break positions** — which word ends each line —
rather than counts, at one width where the two differ. A count says they
disagree; only the position says where.

#### And it weakens §5.3

The screenplay agreed at all 18 widths, and it is **monospaced**: every break
decision sits a whole character-width from the next, so a sub-pixel metric
difference rarely flips one. The mixed fixture is proportional. §5.3's agreement
is therefore weaker evidence than it read as — it is evidence about a forgiving
document, not about the general case. It stands as measured; it does not
generalise.

### 5.5 Where they break — the cause, and the fix (2026-08-04)

§5.4's counts said the two paths disagreed and could not say why. Positions can.
`loki-text/examples/styled_linebreak_lines` prints, for one width, each line's
text, advance and trailing whitespace from the canvas path's retained Parley
layout; the DOM path's answer is a shot of the same width.

*`mixed:family` at 431 px.* The canvas path's third line reads
`"the monospaced words here sits somewhere in "`. The DOM's reads
**`themonospaced words heresits somewhere in the middle`**.

The spaces *across* the run boundaries were gone — the one ending the run before
and the one beginning the run after. Not a metrics difference at all: **missing
characters**.

#### Cause

The text reaches this view as one `<span>` per resolved run, under CSS's default
`white-space: normal`, which collapses and trims whitespace. `loki-layout` does
not: it shapes the model's text as it stands. Two whitespace policies for one
document, and the difference is invisible until a paragraph has a boundary inside
it — which is why §5.3's screenplay (one run per paragraph) never showed it, and
why §5.4's `mixed:plain` control did.

*Fix:* `resolved_para_css` emits **`white-space: pre-wrap`** — "these characters,
wrapped", which is what the canvas path does. `normal` was wrong from the start.

#### After the fix

| fixture | widths agreeing |
| --- | --- |
| screenplay (§5.3) | 18 / 18 |
| `mixed:plain`, `mixed:onerun` | 12 / 12 |
| `mixed:weight`, `mixed:italic`, `mixed:size`, `mixed:family` | 12 / 12 |
| `mixed:charstyle` | 10 / 12 |
| `mixed:spacing` | 8 / 12 |

**Mixing itself is no longer a source of disagreement.** Two cases still flip a
break, and a third control — `mixed:solo-<case>`, the whole paragraph in that
case's properties with no run beside it — separates why:

* `solo-charstyle` (Tinos 14 pt bold, *unmixed*) disagrees at 4 of 14 widths. So
  `charstyle`'s residual is **not** about adjacency; that face at that size
  differs wherever it appears.
* `solo-spacing` agrees at **14 of 14**, and raising the tracking from 1.5 pt to
  6 pt makes the *mixed* case agree at 12 of 12. So letter-spacing is applied on
  both paths and is not dropped at a boundary either.

#### The residual is sub-pixel, and is quantified

`mixed:charstyle` at 299 px: the canvas path's line 3 is
`"styled words here sits somewhere in "`, advance **207.689 pt** against
**224.25 pt** available — 16.56 pt of slack, and the next word (`the`) is about
16.7 pt. The break is marginal by ~0.15 pt. The DOM fits `the`, so its line is
narrower by that much: a difference of order **0.1 %** of a line's advance.

That is the size of the remaining disagreement. It only ever shows where a break
is already within a fraction of a point, which is why it appears at 2–4 widths
out of 12 and not everywhere.

**Not established:** where the 0.1 % comes from. Candidates, none tested: Taffy's
integer-pixel rounding of the inline context's available or measured size; font
sizes that are not a whole number of px (14 pt = 18.667 px — and `charstyle`, the
worse of the two residuals, is the only case with one); variable-font instance
resolution, since the base family resolves to Arimo, a `wght` variable font, and
the canvas path passes explicit normalised coordinates where Stylo resolves
`font-weight` itself.

*What would settle it:* the same instrument one level down — the **advance of a
single line carrying identical text** on both paths, rather than the break it
produces. `styled_linebreak_lines` already prints the canvas side; the DOM side
needs an ink-extent measurement per line, which the band script is one loop away
from.

### 5.6 The 0.1 % was **kerning** (2026-08-05)

§5.5 left a residual of order 0.1 % of a line's advance — enough to flip a break
that was already marginal, too small to read out of a line count. Measured
directly, it has a name.

*The instrument.* `loki-text/examples/advance_probe` renders one row per resolved
style, all carrying the same string, each in a `<span>` emitting **the reflow
view's own CSS**. Each row is `fit-content` on one line, so the row box's width
*is* the run's advance in CSS px; `scripts/sitting/advance_bands.py` measures it
and sets it against the canvas path's advance from
`styled_linebreak_lines`. `LB_FIXTURE=advances:<scale>` multiplies every size,
because a **rounding** difference is a fixed number of pixels and a **metrics**
difference is a fraction of one — and at 12 pt the whole effect is a quarter of a
pixel, which is why §5.5 could not name it.

*What it found.* At scale 6, eleven of twelve cases agreed to within the pixel
the box is rounded up to. One did not: a run in **Tinos Bold** came out 0.13 %
narrower in the DOM, at every scale tested (5, 6, 8) — a fraction, not a
rounding.

Then the controls, each removing one candidate:

| control | result | rules out |
| --- | --- | --- |
| `serifbold` — same face and weight, stated on the run | identical to `emph` | the character-style path |
| `libserifbold` — the *installed* metric-clone, no substitution | identical again | font substitution |
| canvas-side face dump | `Tinos-Bold.ttf`, `synthesis: bold=false` | synthetic bold, wrong face |
| `kernfree` — 46 identical letters, no kern pairs | **agrees** | everything above |
| `kernheavy` — `AV Ta Wo …` | **−7.7 %** | — |

*Cause.* `StyleSpan` carries the document's `w:kern` / `style:letter-kerning`
(gap #23) and `loki-layout` disables the `kern` feature for anything but
`Some(true)`, because Word and LibreOffice default pair kerning **off**. The DOM
path said nothing, and the shaper's default is on. Ordinary prose has few kern
pairs, so it showed as 0.13 %; kern-rich text showed as 7.7 %.

*Fix, and where it had to go.* Stylo 0.8 gates **both** `font-kerning` and
`font-feature-settings` to Gecko, so there is no CSS to say this in — a
declaration is dropped as unknown, which is worse than nothing because it reads
as a fix. The vendored `blitz-dom` now reads a `data-font-features` attribute
into Parley's `font_features` (see `docs/patches.md`), and
`style::span_font_features` emits it. After it, **all twelve advance cases agree
to the box's rounding**, `kernheavy` included (−215 px → +0.8 px), and the mixed
line-break cases `charstyle` and `spacing`'s counterparts follow.

#### The last one: a letter-spaced run beside others (closed 2026-08-05)

`mixed:spacing` disagreed at 4 of 12 widths, and its counts matched the
**untracked** document at every one — a `letter-spacing` run inside a paragraph
with other runs received no tracking at all, while the same run as a paragraph's
only child did.

*Located with the advance probe's structural rows* — the same characters as one
span and as several, with the tracking moved around:

| row | before | after |
| --- | --- | --- |
| one span, tracked | +88 px (correct) | +88 px |
| three spans, middle tracked | **+0 px** | +88 px |
| two spans, **first** tracked | **+288 px** (all 36 chars) | +88 px |
| two spans, **last** tracked | **+0 px** | +200 px |

Read together those say it exactly: the whole inline context took the *first*
span's letter-spacing, and every later span's was ignored.

*Cause.* **Parley 0.6.0** — which the Blitz stack uses — refreshes every field of
a shape item when it breaks a run except `letter_spacing` and `word_spacing`, so
the item keeps `styles[0]`'s for the whole layout. Fixed upstream in **0.10**,
which `loki-layout` already uses: which is why the canvas path was right on the
same document and nothing in either path's own code explained the difference.
Confirmed on the canvas side with the `trackpos` fixture — the same 37 characters
with the tracking nowhere, on the first run and on the middle run give 212.104,
278.104 (+11 × 6 pt) and 284.104 pt (+12 × 6 pt), each exactly the tracked run's
own characters.

*Fix.* `patches/parley` vendors 0.6.0 with the two lines added, scoped by
`version = "0.6.0"` so `loki-layout`'s `^0.10` still resolves from the registry —
`cargo metadata` reports both, which is the check that it stayed narrow. See
`docs/patches.md` for the removal condition.

After it, **every fixture agrees at every width**: the screenplay at 18, and
`plain`, `onerun`, `weight`, `italic`, `size`, `family`, `charstyle` and
`spacing` at 12 each.

### 5.7 Virtualisation, measured (2026-08-05)

§3.2's last open risk: the canvas path rasterises only the tiles on screen, while
the DOM path builds a node per run. What that costs at length.

*Procedure.* `LB_FIXTURE=long:<n>` is *n* numbered paragraphs of ordinary prose,
one style, nothing else — the question is the size of the tree, and anything else
in the document would be a second variable. `scripts/sitting/run.sh
virtualisation` launches the DOM view on each and records the time to the first
frame carrying text (polled at 0.5 s) and the process's peak RSS. The canvas
path's share is `layout_document` in `LayoutMode::Reflow` on the same documents,
timed headlessly.

**Debug build.** The absolute numbers are inflated by it; the scaling and the
ratio between the two paths are the finding.

| paragraphs | DOM: first paint | DOM: peak RSS | canvas: `layout_document` |
| --- | --- | --- | --- |
| 100 | 1.23 s | 374 MB | — |
| 500 | 1.23 s | 390 MB | 0.35 s |
| 2000 | 2.43 s | 447 MB | 1.35 s |
| 8000 | 5.15 s | 676 MB | 4.97 s |

*Reading it.* About 1.2 s of the DOM column is process start and GPU
bring-up — it is the same at 100 and 500 paragraphs, where the document costs
nothing measurable. Above that both columns grow linearly at roughly the same
slope (~0.5–0.6 ms per paragraph), and the DOM path reaches **first paint** no
later than the canvas path finishes the layout it would still have to rasterise.

So the risk as stated does not materialise: what the canvas path virtualises is
rasterisation, and layout — which both pay in full — is the larger term.

**The cost that is real is memory**: 374 MB at 100 paragraphs to 676 MB at 8000,
i.e. **≈ 38 KB per paragraph** on a ~370 MB baseline (wgpu, lavapipe, fonts). For
a `<p>` carrying one `<span>` that is a lot, and it is Blitz's per-node cost
rather than ours.

**Not established:** the canvas path's memory for the same documents, so the
38 KB is a number and not yet a comparison; release-build figures; and scroll
cost once painted, which is where virtualisation would show if it showed
anywhere. *What would settle those:* the same scenario against the canvas path,
which needs the harness to open an arbitrary file — the one thing it cannot do
today.

### 5.9 Lists (2026-08-06)

The last block kind the view rendered as a placeholder. `loki_layout` already
owns the three rules — which marker, what step, what indents — so they were
lifted out of `flow.rs` into `flow_dispatch` (`list_marker`,
`NESTED_INDENT_PT`, `synthesize_list_item_para`) and the DOM path calls them
rather than restating them. The first attempt then rendered the synthesised
paragraph as-is: marker inline, `text-indent: -18pt` for the hang, `tab-size:
18pt` so the marker's tab would land on it.

It disagreed with the canvas path at **24 of 36** swept widths. Three causes,
found in this order:

**1. `text-indent` and `tab-size` do not exist in this stack.** `parley` 0.6's
`TextStyle` carries neither an indent nor a tab stop, and the vendored
`blitz-dom`'s `stylo_to_parley` never reads either property, so both
declarations are dropped as unknown. Photographed at 400 px: every line of every
item — first and continuation alike — began at the same x, and the marker's tab
painted as a `.notdef` box. A declaration that reads as a fix is worse than no
declaration, so `resolved_para_css` now emits no `text-indent` at all and a test
fails if one comes back; the hanging space is a **box** instead
(`style::hanging_row_css`): a flex row indented to `indent_start −
indent_hanging`, a marker cell exactly `indent_hanging` wide, the text in the
rest. Re-measured: markers at x = 33–35, text at 56–57, nested text at 81 —
18 pt and 36 pt from the column, which is what the canvas path's tab stop
resolves to.

**2. The family collector never walked into a list.** `requested_families`
looked at top-level paragraphs only, so a document made entirely of lists
resolved no families, asked Blitz for "Arial" — which this host does not have —
and got its default sans, about **14 %** wider. Every item then wrapped a line
early. This is §5.3's defect a second time, in a second walk; the fix is that
the collector recurses into lists, quotes and table cells, and the guard is a
test per container.

**3. A canvas-path defect the comparison found.** `FlowState::current_indent` is
consumed by `flow_para_place` as a *translation*: an item's non-first blocks are
laid out against the full `content_width` and then shifted right, so their text
overruns the column by one step. On a 565 px column the second paragraph of an
item took two lines on the canvas and three in the DOM, its longest line
423.6 pt against a 405.75 pt measure. **The DOM path is the correct one here**
and the canvas is not; it is left as `TODO(list-indent-measure)` because the
field means two things — an indent inside the measure for a list or a quote, an
absolute page x for a table cell — and separating them moves line breaks in
every document with a quote or a multi-block item, which wants its own sweep.

*Where it stands.* All 36 measured DOM counts are reproduced exactly by the
canvas's own per-paragraph counts under two corrections and no third: (a) the
item's second paragraph measured at `column − indent` rather than `column`, and
(b) the DOM breaking as though its column were **one CSS pixel wider** — its
transition sits one px below the canvas's at every one of the nine transitions
where they differ, and at all nine `DOM(w) = canvas(w+1)`.

**Not established:** the mechanism of (b). It appears on this fixture and not on
`screenplay` or `mixed`, which agree at every width, so the flex row is the
obvious suspect — but expressing the marker cell's width in `px` rather than
`pt` changes nothing, which rules out that conversion. *What would settle it:*
the §5.6 box ruler applied to the row's two cells, reading their resolved widths
directly instead of inferring them from where the text breaks.

### 5.8 Revised sequencing

1. ~~Resolve through `StyleCatalog`.~~ **Done — §5.1.**
2. ~~Route families through `resolve_font_name`.~~ **Done — §5.1a.**
3. ~~Re-run the line-break comparison on a styled document as a measurement.~~
   **Done — §5.3.**
4. ~~Extend it to mixed style runs within a paragraph.~~ **Done — §5.4.**
5. ~~Locate the residual by comparing break positions.~~ **Done — §5.5. The
   cause was CSS whitespace collapsing; fixed. Mixing no longer disagrees.**
6. ~~Measure per-line advances to explain the remaining ~0.1 %.~~ **Done —
   §5.6. It was kerning; fixed in the vendored `blitz-dom`. Every advance case
   now agrees to the pixel the box is rounded to.**
7. ~~Close the one remaining defect: a letter-spaced run beside other runs gets
   no tracking.~~ **Done 2026-08-05 — it was parley 0.6's shaper; see §5.6.
   Every fixture now agrees at every width.**
8. ~~T7.3's per-element scroller.~~ **Done 2026-08-05 — `oversized::AtOversized`,
   with tables and images rendered so it has the elements it exists for. See
   `docs/spec-08-remaining-work.md`.**
9. ~~The virtualisation measurement.~~ **Done — §5.7. The DOM path reaches first
   paint no later than the canvas path finishes laying the same document out;
   memory is the cost that grows.**
10. ~~The image bitmap.~~ **Done 2026-08-05 — there was no defect: the fixture's
    PNG had a bad IDAT CRC, which `image` refuses and ImageMagick accepts, so
    the check that cleared it was more lenient than the decoder under test. The
    `oversized` scenario now counts the figure's own colour in the shot.**
11. ~~Lists.~~ **Done 2026-08-06 — §5.9. Three causes: `text-indent`/`tab-size`
    are inert in this stack (the hanging space is now a box), the family
    collector did not walk into lists, and a canvas-path indent defect the
    comparison found (`TODO(list-indent-measure)`). The residual is one CSS
    pixel of column, unexplained.**
12. Remaining: the editing surface (`TODO(dom-reflow-editing)`) — after which
    the DOM view could replace the canvas one and `loki_renderer::measure` goes
    with it (§3.1, §4).
