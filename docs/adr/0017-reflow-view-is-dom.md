<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0017 — The reflow view is DOM, not canvas

| Field | Value |
| --- | --- |
| Status | **Accepted** — 2026-08-03. Direction agreed; primary risk measured (§3.2); **not implemented** |
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
2. If it holds: build the DOM reflow view behind the existing view-mode switch,
   so both paths are live and comparable.
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
