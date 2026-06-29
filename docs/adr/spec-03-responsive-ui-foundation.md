<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 03 — Responsive UI Foundation

| | |
|---|---|
| **Status** | Draft — pending implementation |
| **Scope** | AppThere Loki (Text); the breakpoint system is designed as shared monorepo UI infrastructure |
| **Sequence** | 3 of 6 — establishes the responsive foundation the Ribbon and Styling Panel build on |
| **Depends on** | Spec 01 (the unified `Viewport`/`LayoutContext` from M4; ADR 0009 layering; design-token location after the uphill-edge fix) |
| **Feeds** | Ribbon (Spec 04 consumes the breakpoint system), Styling Panel (Spec 05 must survive the Compact breakpoint) |

---

## 1. Context & Motivation

Loki runs on desktop and phone today, but it does so by looking *mostly the same at every size and scrolling whatever overflows*. There is no breakpoint system: the layout doesn't adapt, it just spills. The named offender is the **font-substitution warning** — a full-width horizontal band of explanatory text that, on a phone, wraps several times and consumes a large amount of vertical space while laying out illogically for the narrow viewport. But the warning is a symptom; the whole UI needs to be brought up to a deliberate standard of responsive design.

This spec establishes that foundation:

1. A **breakpoint system** with real viewport detection, derived from the single `Viewport` source of truth Spec 01 creates — not a new width source.
2. A principled rule for the **paginated ↔ non-paginated** renderer switch, tied to whether a page actually fits rather than to a device guess.
3. A **redesigned font-substitution warning** that is compact by default and lays out sensibly when narrow.
4. **Typographic polish** for the non-paginated view, which currently renders cramped.
5. A **cross-UI responsive sweep** applying the new foundation to every surface.

The breakpoint system is **shared monorepo UI infrastructure**: Presentation and Spreadsheet need responsive behavior too, so the classification lives in the shared UI layer and all three apps consume it. This spec implements and proves it in Text.

This is **audit-first**. Before building, the implementing agent confirms what Blitz actually supports for responsive layout (media queries vs. programmatic), locates the current font-warning component and the paginated/non-paginated switch, and inventories every surface that misbehaves when narrow. References below are illustrative.

---

## 2. Relationship to Spec 01 (read this first)

Spec 01's audit established the load-bearing fact this spec depends on: **the app already measures the real viewport** via the patched `get_client_rect()`, storing it in `scroll_metrics.client_width` where it feeds reflow — while hit-test/pointer math reads a *separate*, never-written `window_width` signal defaulted to 1280. Spec 01 M4 unifies those into one `Viewport`/`LayoutContext`.

Therefore this spec **does not create or re-specify a width source.** It builds the breakpoint classification *on top of* the unified `Viewport`. If Spec 03 implementation begins before Spec 01 M4 lands, the agent must not introduce yet another width signal — it derives the breakpoint from whatever the current best measured width is and flags the dependency, so the two specs converge on one source rather than adding a third.

Layering and the dependency-direction invariant come from **ADR 0009**; this spec cites it rather than re-proposing a layering. Design tokens are consumed from wherever Spec 01's uphill-edge fix relocates them (the `loki-renderer → appthere_ui` token edge); this spec points at that location, it does not assume the old one.

---

## 3. Goals / Non-Goals

**Goals**

- A `Breakpoint` derived from the unified `Viewport`, exposed as a responsive context/signal to UI components, shared across the three apps.
- A page-fit-driven paginated↔non-paginated switch.
- A font-substitution warning that is compact-by-default, expand-on-demand, dismissible-but-recoverable, and stacks vertically when narrow.
- A non-paginated view with a constrained measure and a responsive type scale.
- Every UI surface audited and fixed for Compact-width behavior, with adequate touch targets.

**Non-Goals**

- Pinch-to-zoom (helpful, not immediately necessary; deferred, but the architecture must leave room — zoom is a `Viewport` property).
- The `Viewport` unification itself (→ Spec 01 M4; consumed here).
- Ribbon responsive *collapse* behavior (→ Spec 04; Spec 03 *provides* the breakpoint system Spec 04 consumes).
- The substitution *engine* and its tests (→ Spec 02; this spec owns only the warning *UI*).
- New chrome features; this is adaptation and polish, not new surfaces.

---

## 4. Working Method

1. **Confirm the Blitz responsive surface.** Determine empirically what the current Blitz/Stylo build supports: do CSS media queries on width work reliably, or must responsive behavior be driven programmatically off the measured viewport? Record the answer — it decides D1's emphasis. Note the standing Blitz constraints that shape every layout choice below: **no `position: fixed`, no `box-shadow`, no CSS custom properties.**
2. **Inventory misbehavior.** Catalogue every surface that wraps, overflows, or wastes space when narrow — the font warning first, then status bar, tab bar, title bar, panels, dialogs.
3. **Build the breakpoint context** atop the unified `Viewport`.
4. **Implement** the page-fit switch, the warning redesign, and the non-paginated typography.
5. **Sweep** the inventoried surfaces against the new foundation.

Standing standards (ADR 0009, Spec 01 conventions) apply throughout; flag deviations rather than conforming silently.

---

## 5. The Breakpoint System

### 5.1 Semantic, not device-named

Breakpoints are **semantic window-size classes**, not device names, because device names lie — tablets, split-screen windows, and large phones in landscape all straddle the categories, and two windows of equal width should behave identically regardless of the hardware they're on. Proposed tiers (to be confirmed against the inventory):

| Class | Width | Intended posture |
|-------|-------|------------------|
| **Compact** | < 600 | Single column, touch-first chrome, non-paginated, stacked panels |
| **Medium** | 600–1024 | Transitional; page-fit decides paginated/non-paginated; panels may overlay |
| **Expanded** | ≥ 1024 | Paginated, full chrome, side-by-side panels |

The classification is derived state: `Viewport.width → Breakpoint`. It is exposed once, via context/signal, so no component re-measures or re-derives.

### 5.2 Source of truth

The `Breakpoint` is computed **only** from the unified `Viewport` (§2). Where Blitz reliably supports CSS media queries, components may use them for purely-visual adjustments — but the programmatic `Breakpoint` signal is the **single source of truth** for any behavior that must be testable (panel collapse, renderer choice, ribbon adaptation). This keeps responsive logic verifiable in the conformance harness without a real window, and independent of how complete Blitz's CSS media-query support turns out to be.

### 5.3 Where it lives

`Viewport` is foundation-layer (Spec 01). The `Breakpoint` classification and the responsive context are **shared UI infrastructure** so Presentation and Spreadsheet inherit them; exact crate placement follows ADR 0009's layering (likely the shared UI crate alongside the relocated design tokens). It must carry no Text-specific assumptions.

---

## 6. Paginated ↔ Non-Paginated Switch

Loki ships two renderers and currently picks between them by a rough size guess. The correct trigger is **content fit, not device class**:

**Decision (D2): switch on whether a page column fits.** Use paginated rendering when a full page width (page size + margins + surrounding chrome) fits at the current zoom; otherwise use the non-paginated renderer to avoid horizontal scrolling. This is computed from the `Viewport` (width, zoom, DPI, page geometry — all already on that type), *not* from the raw `Breakpoint`. A large phone in landscape that can fit a page gets pagination; a narrow split-screen desktop window that can't gets the non-paginated view. The `Breakpoint` still informs *chrome* posture; the *renderer* follows page-fit.

The two interact but are not the same axis, and conflating them is what produces wrong behavior at the edges. The switch must be hysteretic — a small dead-band around the threshold — so a window dragged to exactly the boundary doesn't thrash between renderers.

---

## 7. Font-Substitution Warning Redesign

### 7.1 Current state & the problem

Today the warning is a full-width horizontal band carrying a long explanatory message: which requested fonts weren't resolved, which substitutions were made, and a link to download the originals where the link is known. On a phone this text wraps repeatedly into a tall block, wasting vertical space and laying out illogically for the viewport.

### 7.2 Redesign

**Decision (D3): compact-by-default, expand-on-demand, dismissible-but-recoverable, vertical-stack when narrow.**

- **Default state:** a compact indicator, not a paragraph — e.g. a single concise line or chip ("3 fonts substituted") with an icon. It states the fact and offers to expand. Localized via `fl!()` like all user-facing strings.
- **Expanded state:** a structured **missing → substitute → action** view. On Expanded width this can be a compact table; on Compact width it becomes a **vertical stack of cards**, one per substitution, each showing the requested font, the substitute used, and a download-original action where a link is known. No horizontal band that wraps.
- **Severity awareness:** a metric-compatible substitution (Carlito for Calibri — the §7.3 set from Spec 02) is low-concern and can be styled calmly; a fallback that materially changes metrics is higher-concern and surfaced more prominently. This reuses the substitution-engine signal Spec 02 exercises; the warning is its UI.
- **Dismissible but recoverable:** the user can dismiss it (remembered per document), and recover it from a persistent, unobtrusive indicator (e.g. in the status bar) so dismissing isn't losing the information.
- **Blitz-aware presentation:** no `position: fixed` (the indicator lives in document/chrome flow, not pinned to the viewport), no `box-shadow` (elevation via border/background from the token set), no custom properties (theming through the token system at its post-Spec-01 location).

---

## 8. Non-Paginated View Typography

The non-paginated view exists to reduce horizontal scrolling on small screens, but it currently renders **cramped**, which is a readability bug.

**Decision (D5): constrained measure + responsive type scale.**

- **Bounded measure:** cap line length to a comfortable reading measure (roughly 45–75 characters) rather than letting text run the full viewport width; center the column with breathing room when the viewport exceeds the measure.
- **Adequate vertical rhythm:** line-height, paragraph spacing, and margins/padding tuned for reading, not packed.
- **Responsive scale:** type and spacing scale sensibly across `Breakpoint` classes so Compact isn't a shrunk Expanded.
- **Reflow within bounds:** reflow to viewport width with min/max bounds, so the view stays readable from phone to wide window.

This view is what most users see on Compact, so its polish disproportionately affects perceived quality.

---

## 9. Cross-UI Responsive Sweep

With the foundation in place, apply it to every surface the §4 inventory flagged. At minimum: status bar, tab bar (`AtTabBar`), title bar (`AtTitleBar`), status bar (`AtStatusBar`), any dialogs/panels, and the home/document surfaces. Each must: lay out logically at Compact (stack, don't wrap-and-spill), present **touch-sized targets** (≈44px minimum) when the breakpoint is touch-first, and respect the Blitz constraints. The Ribbon is explicitly handed to Spec 04, which consumes this spec's breakpoint system rather than reinventing it.

---

## 10. Key Decisions (ADR-style)

**D1 — Programmatic breakpoint signal is the source of truth.** Derived from the unified `Viewport`; CSS media queries used only for purely-visual tweaks where Blitz supports them. Rationale: testable without a window, independent of Blitz CSS completeness, reuses existing measurement. Tradeoff: some adaptation is in Rust rather than CSS — accepted for verifiability.

**D2 — Renderer switch follows page-fit, not breakpoint.** Content fit is the right trigger; device class is a proxy that's wrong at the edges. Hysteresis prevents thrash. Tradeoff: a slightly more involved computation than a width threshold — worth it for correct edge behavior.

**D3 — Warning is compact-by-default and stacks when narrow.** Directly fixes the named vertical-space problem; dismissible-but-recoverable preserves the information. Tradeoff: an expand interaction the old band didn't have — accepted; the band's "always fully expanded" was the bug.

**D4 — Semantic window-size classes, not device names.** Equal-width windows behave equally; categories don't break on tablets/split-screen. Tradeoff: names are less intuitive than "phone/tablet" — mitigated by documenting the mapping.

**D5 — Non-paginated view gets a bounded measure and responsive scale.** Cramped text is a readability defect; bounded measure is basic typography. Tradeoff: text no longer uses full width on wide windows — that's the point.

---

## 11. Milestones & Acceptance Criteria

**M1 — Breakpoint foundation.** Blitz responsive-surface question answered and recorded; `Breakpoint` derived from the unified `Viewport`; responsive context exposed; placed per ADR 0009 as shared UI infra. *Accept:* a width change produces the correct `Breakpoint` in a test without a real window; no second width source introduced.

**M2 — Page-fit renderer switch.** Paginated↔non-paginated driven by page-fit with hysteresis. *Accept:* a landscape-phone-width that fits a page renders paginated; a narrow desktop window that doesn't renders non-paginated; dragging across the boundary doesn't thrash.

**M3 — Font-warning redesign.** Compact indicator, expand-on-demand missing→substitute→action view, vertical-stack at Compact, dismiss + status-bar recovery, severity-aware styling, `fl!()` strings, Blitz-constraint-clean. *Accept:* on a Compact viewport the warning occupies a small fixed footprint and expands to a logical stacked layout, not a multiply-wrapped band; dismiss and recovery both work.

**M4 — Non-paginated typography.** Bounded measure, vertical rhythm, responsive scale, bounded reflow. *Accept:* text in the non-paginated view holds a comfortable measure across Compact→Expanded and no longer reads cramped.

**M5 — Cross-UI sweep.** Every inventoried surface adapted; touch targets sized at touch-first breakpoints. *Accept:* the §4 inventory is empty of wrap-and-spill cases; the Ribbon is left to Spec 04 but already consumes the M1 breakpoint system.

---

## 12. Out of Scope

- Pinch-to-zoom (deferred; zoom remains a `Viewport` property so it can be added without rework).
- The `Viewport` unification (→ Spec 01 M4).
- Ribbon collapse/overflow behavior (→ Spec 04; it consumes the M1 breakpoint system).
- The font-substitution *engine* and its conformance tests (→ Spec 02; this spec owns the warning UI only).
- Styling-panel internals (→ Spec 05; it must survive Compact, which this spec's foundation enables).
