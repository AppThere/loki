<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 04 — Ribbon UI Refinement

| | |
|---|---|
| **Status** | Draft — pending implementation |
| **Scope** | AppThere Loki (Text); the ribbon framework is designed as shared monorepo UI infrastructure |
| **Sequence** | 4 of 6 — consumes the Spec 03 breakpoint system |
| **Depends on** | Spec 03 (breakpoint context, the Expanded-by-default contract, the R-13e/R-14 inputs); Spec 01 (ADR 0009 layering, design-token location) |
| **Feeds** | Styling Panel (Spec 05 — the Write tab's Styles group is the entry point into the styling panel) |

---

## 1. Context & Motivation

The ribbon is half-built and internally inconsistent. The current first tab ("Home") looks like a plain icon-button toolbar; the Publish tab uses **labeled controls grouped into labeled sections** — the look the whole ribbon should have. On top of the visual split, there are three structural problems:

1. **A naming collision.** There is a "Home" tab *in the ribbon* and a "Home" screen *for the application* (document/file management). Two different things share a name.
2. **No deliberate responsiveness.** The ribbon currently just scrolls horizontally when it overflows. That's an acceptable floor but not a design — it should adapt before it resorts to scrolling.
3. **Missing tabs and a creation gap.** There's no Insert tab, and the editor can *render* objects (images, tables, math, headers/footers, hyperlinks, partially footnotes) it gives the user no way to *create*.

This spec makes the ribbon consistent, responsive, and complete:

- Rename the ribbon's first tab **Home → Write**, resolving the collision.
- Standardize every tab on the **labeled-group** style (Publish's look), retiring the icon-only style.
- Build a **progressive collapse cascade** (full groups → condensed → overflow menu) with horizontal scroll kept only as the guaranteed last-resort fallback.
- Define the full tab set — **Write, Insert, Layout, References, Review** plus contextual **Table** and **Picture** tabs.
- Gate the **Insert** tab behind a **render-capability audit**: expose creation UI *only* for objects the renderer verifiably handles, so no button creates something the app can't draw and no button does nothing.

The ribbon framework (tab strip, group container, collapse engine, overflow, contextual-tab mechanism) is **shared monorepo UI infrastructure** — Presentation and Spreadsheet have ribbons too. This spec builds the framework in the shared layer and Loki Text's specific tab *content* on top of it.

This is **audit-first**: inventory the existing tabs/groups/controls, run the render-capability audit, and confirm Blitz's layout surface before building. References below are illustrative.

---

## 2. Relationship to Spec 03 (read this first)

Spec 03 shipped the breakpoint system this spec consumes. Two facts from that implementation are now contracts:

- **`use_breakpoint()` is resilient and defaults to `Breakpoint::Expanded`** when no responsive context is present (via `try_consume_context`). The ribbon is persistent chrome, so in Loki Text it *will* have the context — but the shared framework must behave correctly at Expanded-by-default so Presentation/Spreadsheet (which don't yet wire the context) get a sane full-chrome ribbon rather than a broken one.
- **Tiers are Compact < 600, Medium 600–1024, Expanded ≥ 1024.** The collapse cascade (§7) keys off these *as defaults* but is ultimately **width-driven** (see D3) — it does not assume a tier has room, it measures.

Two Spec 03 findings were explicitly handed forward and are **named inputs** here, not new discoveries:

- **R-13e — ribbon select width.** The ribbon's select/dropdown controls (font family, size) misbehave on width; the condensed state must handle their sizing (§9).
- **R-14 — tab-strip touch height.** The tab strip needs touch-sized targets at touch-first breakpoints (§9).

Layering and the dependency-direction invariant come from **ADR 0009**. Design tokens are consumed from their post-Spec-01 location (the relocated `loki-renderer → appthere_ui` token edge). The ribbon observes editor **selection state** to drive contextual tabs — a downhill UI→model dependency, permitted by ADR 0009.

Standing Blitz constraints still bind every layout choice: **no `position: fixed`, no `box-shadow`, no CSS custom properties.**

---

## 3. Goals / Non-Goals

**Goals**

- A shared ribbon framework: tab strip, labeled-group container, width-driven collapse engine, overflow menu, contextual-tab mechanism.
- One consistent visual standard — labeled groups everywhere.
- Home→Write rename with the application Home screen left distinct.
- The full tab set with logical, audited control organization.
- An Insert tab whose every control is backed by a verified render *and* create capability; a committed capability table classifying every object.
- Touch-appropriate posture at Compact (R-14, R-13e).

**Non-Goals**

- New rendering capability. This spec exposes creation UI for what the renderer *already* handles; it does **not** add renderer support for new object types. (If an object is render-only, it stays creation-less here.)
- The styling panel itself (→ Spec 05). The Write tab's Styles group is only the *entry point*.
- Building a math equation editor, a shape-drawing surface, or any large new creation surface — those are flagged by the gate as their own future specs if the audit finds them render-only.
- The substitution/conformance work (→ Spec 02), though the render-capability gate *should* lean on Spec 02 goldens as evidence where they exist.

---

## 4. Working Method

1. **Inventory the ribbon.** Enumerate current tabs, groups, and controls; note which use the icon-only style vs. labeled groups; locate `AtRibbon` and the tab content.
2. **Run the render-capability audit (§10).** For each insertable object, determine import/render/create status empirically — leaning on Spec 02 goldens as render evidence where they exist. Produce the capability table.
3. **Confirm the Blitz layout surface** for the collapse engine (how width is measured, what overflow mechanisms are reliable).
4. **Build the framework**, then Loki Text's tab content, then the collapse cascade, then contextual tabs.
5. **Apply touch posture** (R-14, R-13e) at Compact.

---

## 5. Naming

**Decision (D1): the ribbon's first tab is renamed Home → Write; the application Home screen keeps its name.** "Write" is chosen to hold the most-used writing controls (D5). This resolves the two-Homes collision: *Home* now unambiguously means the document-management screen, *Write* is the primary ribbon tab. Both are user-facing strings, localized via `fl!()`.

---

## 6. The Ribbon Framework (shared infrastructure)

Built in the shared UI layer (with `AtRibbon`), consumed by all three apps. Provides:

- **Tab strip** — the row of tab labels, touch-sizable (R-14), with the contextual-tab slots.
- **Group container** — a labeled section holding controls, with declared collapse behavior (§7).
- **Collapse engine** — measures available width and collapses groups by priority (§7).
- **Overflow menu** — a "more" affordance holding groups that don't fit.
- **Contextual-tab mechanism** — shows/hides a tab in response to a selection signal.

Each app supplies its own tab/group/control *content*; the framework carries no Text-specific assumptions. This mirrors the shared-infra-plus-consumer pattern of Specs 02 and 03.

---

## 7. Progressive Collapse Cascade

**Decision (D3): collapse is width-driven, not tier-driven.** The breakpoint sets defaults, but the engine measures actual available width and collapses groups by declared priority until they fit. This directly answers the open question of whether Expanded (≥1024) "has room" for full labeled groups: it doesn't assume — it tries full, and condenses if it doesn't fit. The cascade, per group, in order:

1. **Full** — labeled group, full-size controls, group label visible.
2. **Condensed** — controls pack tighter; the group label may drop; low-priority controls within the group may merge into a dropdown. (R-13e's select-width handling lives here.)
3. **Overflow** — the whole group moves into the overflow ("more") menu.
4. **Scroll fallback** — only when even overflow can't fit does the strip fall back to horizontal scroll (today's behavior, retained as the guaranteed floor, never the first resort).

Each group declares a **collapse priority** (which groups condense/overflow first) and a defined condensed and overflow representation, so collapse is deterministic and not layout-engine-incidental. Collapse must be hysteretic at the boundaries to avoid thrash when a window is resized across a fit threshold (same principle as Spec 03's renderer switch).

---

## 8. Visual Standard

**Decision (D2): labeled groups everywhere; retire the icon-only style.** The Publish tab's labeled-group look becomes the single standard; the icon-only Home/Write style is removed. Consistency across tabs is the goal, so a control looks and behaves the same wherever it appears. Within the Blitz constraints, group separation and elevation come from borders/background from the token set (no `box-shadow`), and nothing is pinned (`no position: fixed`).

---

## 9. Tab Inventory & Organization

Proposed organization, to be confirmed and mapped against the §4 inventory (the agent maps existing controls to groups; it does not invent controls):

- **Write** (was Home) — the daily driver: Clipboard, Font, Paragraph, Styles (the Spec 05 entry point), Editing (find/replace). The most-used writing controls.
- **Insert** — objects, **each gated by §10**: images, tables, math, header/footer, hyperlink, footnote (gated), shape (gated). See §10.
- **Layout** — page setup (size, orientation, margins), columns, spacing, breaks.
- **References** — footnotes/endnotes management, table of contents, captions, citations. (Footnote *creation* conventionally lives here; it remains gated by §10's render-capability finding.)
- **Review** — spelling/grammar, word count, language, comments, track changes (if supported), accessibility.
- **Contextual Table** — appears on table selection: row/column insert-delete, merge, borders, table styles.
- **Contextual Picture** — appears on image selection: size, wrap, alt text, position.

**Publish reconciliation:** the existing Publish tab (export — PDF/X, EPUB, etc.) is the *source* of the target visual standard and isn't in the writing-tab set above. It should be **retained as its own tab**; export is a distinct concern and folding it into the others would crowd them. The audit confirms nothing in Publish belongs better elsewhere.

**Touch posture (R-14, R-13e):** at Compact / touch-first breakpoints, the tab strip and controls take touch-sized targets (≈44px, consistent with Spec 03's `TOUCH_MIN`), and the ribbon may adopt the existing **bottom-ribbon-for-touch** placement so controls are thumb-reachable. R-13e's select-width handling applies in the condensed state. The essential-controls subset shown at Compact follows from group collapse priority (§7), not a separate hand-maintained list.

---

## 10. The Insert Tab & Render-Capability Gate

The Insert tab's defining rule: **a creation control may exist only if the renderer verifiably handles the object it creates.** This prevents two failure modes — a button that creates something the app can't draw, and a dead button that does nothing.

### 10.1 The audit

For each insertable object, determine three things empirically (leaning on Spec 02 goldens as render evidence where available):

- **Import** — does the importer parse it into the model?
- **Render** — does the renderer draw it correctly?
- **Create** — is there a model-construction path to make a new one?

### 10.2 The classification

Each object lands in exactly one bucket:

| Bucket | Condition | Insert-tab treatment |
|--------|-----------|----------------------|
| **Create-ready** | renders + has (or can cheaply get) a create path | Full creation control exposed |
| **Render-only** | renders (imported docs display it) but no/incomplete create path | **No creation control** — no dead UI; tracked as creation-pending |
| **Unsupported** | renderer can't draw it | Out of scope; tracked |

Render-only objects get **no control at all** rather than a disabled one — a disabled button is still clutter and still implies a promise. Imported documents containing them still display correctly; we simply don't offer to author them yet.

### 10.3 Expected inputs (to be resolved by the audit, not pre-decided)

From the maintainer's current knowledge, seeding the audit (the audit confirms or corrects each):

- **Images, tables, headers/footers, hyperlinks** — render confirmed; expected **create-ready** (images need the existing `loki-file-access` picker).
- **Math** — renders; **create** likely needs an equation-input surface. If the audit finds no cheap create path, math is **render-only** and an equation editor becomes its own future spec.
- **Footnotes** — renders "to an extent"; the audit determines whether it's create-ready or render-only based on how complete the render path is.
- **Shapes** — render status **unknown**; if the renderer can't draw shapes, **unsupported** (no creation control), and shape support becomes a future renderer spec before any Insert control appears.

The capability table is a committed deliverable, so the Insert tab's contents are *derived from verified capability*, not from a wishlist.

---

## 11. Key Decisions (ADR-style)

**D1 — Home→Write; app Home screen unchanged.** Resolves the two-Homes collision with the writing-focused name. Tradeoff: users used to "Home" relearn one label — minor, and the collision was worse.

**D2 — Labeled groups everywhere.** One consistent standard (Publish's), icon-only retired. Tradeoff: icon-only is denser — but consistency and labels' discoverability win, and the collapse cascade recovers density when space is tight.

**D3 — Collapse is width-driven, not tier-driven.** The engine measures and condenses by priority; the breakpoint only sets defaults. Tradeoff: a measuring collapse engine is more work than CSS breakpoints — required, because the ribbon can overflow even at Expanded.

**D4 — Insert controls are gated by verified render+create capability; render-only gets no control.** No button draws the undrawable; no dead buttons. Tradeoff: some objects users might want to insert (math, maybe shapes) may be absent at first — honest, and each becomes a tracked future spec rather than a broken control.

**D5 — Write holds the most-used controls.** The first tab is the daily driver. Tradeoff: deciding "most-used" is a judgment — grounded in the §4 inventory and convention, confirmable later.

---

## 12. Milestones & Acceptance Criteria

**M1 — Framework + rename.** Shared tab strip, labeled-group container, overflow scaffold, contextual-tab mechanism; Home→Write; `fl!()` strings. *Accept:* the framework renders a sane full-chrome ribbon at Expanded-by-default with no responsive context (Presentation/Spreadsheet path); the two-Homes collision is gone.

**M2 — Visual standardization.** Every tab on the labeled-group standard; icon-only style removed. *Accept:* no tab uses the retired style; controls look/behave identically across tabs within Blitz constraints.

**M3 — Collapse cascade.** Width-driven full→condensed→overflow→scroll with per-group priority and hysteresis; R-13e select-width handled in condensed. *Accept:* narrowing the window condenses then overflows groups by priority before any horizontal scroll appears; resizing across a threshold doesn't thrash.

**M4 — Render-capability gate + Insert tab.** Committed capability table; Insert tab exposes only create-ready controls; render-only objects have no control. *Accept:* every Insert control maps to a create-ready row in the table; no control exists for a render-only/unsupported object; imported docs with render-only objects still display.

**M5 — Remaining tabs + contextual.** Layout, References, Review populated from the inventory; Table/Picture appear on selection and dismiss on deselection. *Accept:* selecting a table shows the Table tab and hides it on deselection; same for Picture; no tab invents controls absent from the inventory.

**M6 — Touch posture.** Tab strip and controls touch-sized at Compact (R-14); bottom-ribbon-for-touch placement where applicable; Compact control set follows collapse priority. *Accept:* at Compact, targets meet `TOUCH_MIN`; the Compact ribbon is usable by thumb without horizontal scrolling for the essential set.

---

## 13. Out of Scope

- Adding renderer support for any object (this spec only exposes creation for what already renders).
- Building an equation editor, shape-drawing surface, or other large creation surface flagged render-only by the gate — each becomes its own future spec.
- The styling panel internals (→ Spec 05); the Write Styles group is only the entry point.
- Conformance/substitution work (→ Spec 02), though the gate uses its goldens as render evidence.
- Benchmarking the ribbon's layout cost (→ Spec 06).
