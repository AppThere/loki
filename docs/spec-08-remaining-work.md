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
