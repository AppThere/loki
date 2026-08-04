<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0017 — The reflow view is DOM, not canvas

| Field | Value |
| --- | --- |
| Status | **Accepted** — 2026-08-03. Direction agreed; view built behind a flag (§5). **Open defect:** mixed-run paragraphs wrap differently from the canvas path (§5.4) |
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

**Two rendering paths remain**, since paginated stays on the canvas. Anything
shared — spell state, revision display, the measure — is consumed by both, so
this trades one hazard (ambient state crossing passes) for another (two paths
that must agree about the document).

### 3.3 Sequencing

1. ~~The line-break comparison in §3.2.~~ **Done 2026-08-03 — the paths agree.**
   Extend it to mixed style runs before relying on it further.
2. ~~Build the DOM reflow view behind the existing view-mode switch, so both
   paths are live and comparable.~~ **Done 2026-08-03 — see §5.**
3. Move T7.3's per-element scroller onto it.
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

#### Fixed: a boundary with no formatting behind it

`content::coalesce` now joins adjacent resolved spans whose emitted CSS is
identical, so a run split that carries no formatting meaning — and DOCX is full
of them, since a run boundary survives spell state and revision ids — no longer
reaches the renderer. After it, `mixed:plain` agrees at all 12 widths and the
screenplay (§5.3) still agrees at all 18.

Compared on the **emitted CSS** rather than on the `StyleSpan`s: the CSS is what
reaches the renderer, so two spans differing only in something this view does not
emit are one span as far as line breaking goes.

#### Not established, and what would settle it

The residual — genuinely different adjacent runs — is **unexplained**. Candidates,
none of them tested: Blitz rounding each inline box's measurement at the
boundary; a difference in how the two set up Parley across a style change; and
variable-font instance selection (the base resolves to Arimo, a `wght` variable
font, and the canvas path passes explicit normalised coordinates where Stylo
resolves `font-weight` itself).

*What would settle it:* compare **break positions** — which word ends each line —
rather than counts, at one width where the two differ. A count says they
disagree; only the position says where, and the candidates above predict
different places.

#### And it weakens §5.3

The screenplay agreed at all 18 widths, and it is **monospaced**: every break
decision sits a whole character-width from the next, so a sub-pixel metric
difference rarely flips one. The mixed fixture is proportional. §5.3's agreement
is therefore weaker evidence than it read as — it is evidence about a forgiving
document, not about the general case. It stands as measured; it does not
generalise.

### 5.5 Revised sequencing

1. ~~Resolve through `StyleCatalog`.~~ **Done — §5.1.**
2. ~~Route families through `resolve_font_name`.~~ **Done — §5.1a.**
3. ~~Re-run the line-break comparison on a styled document as a measurement.~~
   **Done — §5.3.**
4. ~~Extend it to mixed style runs within a paragraph.~~ **Done — §5.4. They do
   not agree; one cause found and fixed, the residual open.**
5. Locate the residual by comparing break *positions* at a disagreeing width
   (§5.4). Until then, the ADR's direction stands but its fidelity claim does
   not: the DOM reflow view wraps mixed-run paragraphs differently from the
   canvas one.
6. Then T7.3's per-element scroller, and the virtualisation measurement.
