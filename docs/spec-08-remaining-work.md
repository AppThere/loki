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

**Follow-up, not done:** the ~25 per-component declarations are now redundant
(all name the same token). Removing them is a one-fact-one-derivation cleanup
with a wide diff and no behaviour change, so it is a separate pass.

Not fixed, and not this branch's: the macro trust/signature stack, `loki-layout`'s
squiggle clamp, and the gate-script bypasses.

---

## Phase 6 — Page styles (I-04)

**Re-scoped XL → M by S0.5.** ADR-0012 Decision 2 already shipped `Length<Emu>`, `PageStyle`, the N-column model, ODT master-page round-trip, DOCX section export and `mirror_margins`. **Read ADR-0012 and confirm against the tree before writing anything** — this phase's scope was wrong by a wide margin once already.

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
