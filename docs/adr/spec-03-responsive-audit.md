<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 03 — Responsive UI Foundation: Audit Report

| | |
|---|---|
| **Status** | Audit complete; triaged → **M1 (Breakpoint foundation) implemented**. M2–M5 pending, audit-first per finding. |
| **Method** | Audit-first per Spec 03 §4. Confirms the Blitz responsive surface, locates the font-warning component and the paginated↔non-paginated switch, and inventories every chrome surface that misbehaves when narrow. |
| **Companion** | [spec-03-responsive-ui-foundation.md](spec-03-responsive-ui-foundation.md) (the design spec) |
| **Precedent** | Same audit-then-triage flow as [spec-01-audit-report.md](spec-01-audit-report.md) and [spec-02-conformance-inventory.md](spec-02-conformance-inventory.md). |

This report establishes ground truth and a finding register (R-1 … R-15). It
makes **no code changes** — like Spec 01/02, implementation waits for the
maintainer to triage the findings and confirm the breakpoint tiers and crate
placement.

---

## 1. Executive summary

- **The Blitz responsive-surface question (Working Method §4.1) is answered: CSS
  `@media` width queries are *not used anywhere* in production, so D1 holds —
  the programmatic `Breakpoint` signal must be the single source of truth.** The
  three standing Blitz constraints (`position: fixed`, `box-shadow`, CSS custom
  properties) are real and *already observed*: **zero** instances of any of them
  in `loki-text` / `appthere-ui` / `loki-renderer`.
- **The Spec 01 dependency is *partially* landed.** The width source is unified
  (`Viewport`, one measured value) — the load-bearing fact Spec 03 relies on is
  true. But `Viewport` carries **only** `inner_width_px` (no zoom / DPI / page
  geometry), and it lives in the **app layer** (`loki-text/src/editing/`), not in
  shared UI infrastructure. Both must change for Spec 03 (R-2, R-3).
- **There is no breakpoint system, and width thresholds are already
  fragmented.** Three different magic numbers decide responsive behaviour today:
  `BREAKPOINT_DESKTOP_PX = 768` (home tab), `REFLOW_BREAKPOINT_PX = 900`
  (renderer switch), and Spec 03 proposes 600/1024. Unifying these is exactly
  M1's job (R-4).
- **The named offender is confirmed.** The font-substitution warning is a
  full-width flex-row band that never stacks, has no severity model, and no
  status-bar recovery (R-9 … R-12).
- **The renderer switch is a bare width guess** (`width < 900`), with **no
  page-fit, no zoom/DPI, and no hysteresis** on the mode boundary (R-7, R-8).
  The constant doesn't even match its own comment (cites an 816 px page, uses
  900).
- **Exactly one component is responsive today** (`AtHomeTab`). Title bar and
  status bar wrap-and-spill; several panels have fixed widths that break narrow;
  the ribbon tab strip is below the 44 px touch minimum (R-13, R-14).

**Readiness:** Spec 01 M4's *width* unification is done; **Spec 03 M1–M5 have not
started.** No second width source must be introduced (Spec 03 §2).

---

## 2. Working Method §4.1 — the Blitz responsive surface (answered)

| Question | Finding | Evidence |
|---|---|---|
| Do CSS `@media` width queries work / get used? | **Not used.** 0 occurrences in `loki-text`, `appthere-ui`, `loki-renderer`. Responsive behaviour must be driven programmatically off the measured `Viewport`. | `grep -rn "@media" --include=*.rs` → 0 |
| `position: fixed`? | **Unsupported & unused.** Collapses to `absolute` in `stylo_taffy`. | `CLAUDE.md` "position: fixed … collapses to absolute"; 0 uses in scope |
| `box-shadow`? | **Unused in scope.** (1 instance in `loki-presentation`, out of Text scope.) Elevation must come from border/background tokens. | 0 in `loki-text`/`appthere-ui`/`loki-renderer` |
| CSS custom properties (`var(--`)? | **Unused.** Theming flows through the Rust token constants. | 0 `var(--` in scope |
| `position: absolute`? | **Confirmed working** (block-level, in a positioned ancestor) — the spell panel relies on it. | `CLAUDE.md` 2026-06-28 note; `editor_spell_panel.rs` |

**Conclusion (decides D1's emphasis):** the breakpoint must be a **programmatic
signal**, not CSS media queries. This is also what keeps it testable without a
real window (Spec 03 M1 acceptance) and aligns with the conformance-harness
philosophy from Spec 02. CSS media queries are *not currently available* as even
a secondary visual-tweak channel — if ever wanted, their support must be
verified at runtime first and marked `// COMPAT(dioxus-native):`.

---

## 3. Spec 01 dependency status (read with Spec 03 §2)

| Item | Status | Detail / file |
|---|---|---|
| **One width source, not two** | ✅ **Landed** | The stale `window_width` (defaulted 1280, never written) is deleted; everything reads the measured `scroll_metrics.client_width` via `Viewport`. Writer: `editor_canvas.rs:256` (`onscroll` → `client_width`). Readers: `editor_pointer.rs`, `editor_keydown.rs`, `document_view.rs`. |
| **`Viewport` type exists** | ✅ **Landed (minimal)** | `loki-text/src/editing/viewport.rs` — `struct Viewport { inner_width_px: f32 }` + `new` + `centred_origin_x`. |
| **`Viewport` carries zoom / DPI / page geometry** | ❌ **No** | Only `inner_width_px`. The type's own doc comment says *"zoom / DPI can join this type when Spec 03 (Responsive) needs them"* and page geometry stays in `DocumentState`. **D2 (page-fit) needs zoom + page geometry on/through this type — R-2.** |
| **`LayoutContext`** | ❌ **Not created** | The Spec 01 audit mentioned `Viewport`/`LayoutContext`; only `Viewport` exists. No blocker — Spec 03 builds on `Viewport`. |
| **`Viewport` lives in shared UI infra** | ❌ **No — app layer** | It is in `loki-text/src/editing/`. Spec 03 §5.3 requires the breakpoint (and ideally the viewport) be shared so Presentation/Spreadsheet inherit it. **R-3.** |
| **Design tokens & the uphill-edge fix** | ✅ **Landed** | Tokens in `appthere-ui/src/tokens/` (`colors`/`layout`/`spacing`/`typography`). `loki-renderer` no longer depends on `appthere_ui`; geometry tokens are injected via props. The breakpoint classification should sit *alongside* these tokens. |

**Directive for the implementer (Spec 03 §2):** do **not** add a third width
signal. Extend the existing `Viewport` (zoom/DPI) and *relocate* it + the new
`Breakpoint` into shared UI, rather than re-measuring.

---

## 4. Threshold fragmentation (the M1 unification target)

Three independent magic numbers already gate "responsive" behaviour, none of
them agreeing:

| Constant | Value | Used by | File |
|---|---|---|---|
| `BREAKPOINT_DESKTOP_PX` | **768** | `AtHomeTab` row↔column switch (the *only* responsive component) | `appthere-ui/src/tokens/layout.rs:66` |
| `REFLOW_BREAKPOINT_PX` | **900** | paginated↔reflow renderer default | `loki-text/.../editor_inner.rs:77` |
| Spec 03 proposed tiers | **600 / 1024** | (not yet implemented) | spec §5.1 |

The 900 constant's own comment justifies it by "a US-Letter page (~816px) plus
margins no longer fits" — i.e. it is reaching for a **page-fit** rule (D2) but
hard-coding a guess instead (R-7). M1 should replace 768 and 900 with the
single derived `Breakpoint`, and M2 should replace 900 with a real page-fit
computation. **R-4.**

---

## 5. Misbehavior inventory (Working Method §4.2)

Verdict legend: **OK** (adapts/scrolls sensibly) · **SPILL** (wrap-and-spill or
overflow) · **WASTE** (wastes vertical/horizontal space when narrow) ·
**FIXED** (fixed dimension breaks narrow) · **TOUCH** (sub-44 px target).

| # | Surface | File | Layout today | Narrow (<600) verdict |
|---|---|---|---|---|
| R-9 | **Font-substitution warning** | `loki-text/.../editor_inner.rs:779–861` | full-width `flex-direction: row`, space-between | **SPILL / WASTE** — never stacks; long message wraps into a tall band (the named offender) |
| R-13a | **Title bar** `AtTitleBar` | `appthere-ui/.../title_bar.rs:72–174` | `flex row`, centred title `flex:1`, right-side app name + collaborator badge | **SPILL** — no width guard; right cluster wraps when narrow |
| R-13b | **Status bar** `AtStatusBar` | `appthere-ui/.../status_bar.rs:46–167` | `flex row`, fixed 16px gaps, left/right sections, height 24px | **WASTE/SPILL** — rigid; no reflow; content crowds/clips when narrow |
| R-13c | **Tab bar** `AtTabBar` | `appthere-ui/.../tab_bar.rs:70–175` | `flex row`, `overflow-x: auto` | **OK** — horizontal scroll |
| R-13d | **Home tab** `AtHomeTab` | `appthere-ui/.../home_tab/mod.rs:70–231` | row↔column at 768 px | **OK** — *the only responsive component* |
| R-13e | **Ribbon** (strip/content/group/button/select) | `appthere-ui/.../ribbon/*` | `flex row` + `overflow-x: auto`; buttons 44×44; select fixed **180px** | **OK (scroll)** except **FIXED** select width; *responsive collapse is Spec 04's* |
| R-14 | **Ribbon tab strip height** | `appthere-ui/.../ribbon/tab_strip.rs` | strip height **36px** | **TOUCH** — below 44 px (documented TODO in the file) |
| R-13f | **Spell suggestion panel** | `loki-text/.../editor_spell_panel.rs:37–201` | `position: absolute`, 300px, clamped to viewport | **OK** (horizontal clamp); item touch height unverified |
| R-13g | **Metadata panel** | `loki-text/.../editor_metadata_panel.rs:40–155` | docked, form rows with **fixed 140px** label | **FIXED** — label+input clip/wrap below ~250 px |
| R-13h | **Language panel** | `loki-text/.../editor_language_panel.rs:28–168` | docked, fixed height 200px, space-between rows | **OK-ish** — wraps if space allows; action-button touch unverified |
| R-13i | **Document tab** `AtDocumentTab` | `appthere-ui/.../document_tab.rs:90–159` | `max-width: 140px`, ellipsis | **OK** — truncates; scrolls in the tab bar |

**Global:** no media queries, no breakpoint framework, no `compact`/`is_narrow`
conditionals outside `AtHomeTab`. Touch targets are 44 px almost everywhere
(WCAG 2.5.8 is documented per-component) — the exception is the ribbon tab strip
(R-14) and three unverified panel action-button heights (R-15).

---

## 6. Font-substitution warning — current state (M3 baseline)

| Aspect | Today | Gap vs Spec 03 §7 |
|---|---|---|
| Form | Inline full-width `flex row` band, `editor_inner.rs:779–861` | Needs compact-by-default chip + expand-on-demand (D3) |
| Narrow layout | Wraps in place; never stacks | Needs vertical card stack at Compact |
| Data | `FontResources.substitutions: HashMap<String, Option<String>>` (`Some` = substitute, `None` = no substitute) — `loki-layout/src/font.rs:36` | Sufficient for missing→substitute→action, but… |
| Severity | **None** — metric-compatible (Carlito↔Calibri) and material fallbacks are formatted identically | D3 wants severity-aware styling; needs a signal on the substitution result |
| Dismiss | Yes — `dismiss_font_warning: Signal<bool>`, cleared on new doc, persists across tab restore | OK; D3 also wants… |
| Recovery | **None** — once dismissed it's gone until reload | Needs a persistent status-bar indicator to recover |
| i18n | ✅ `fl!()`, keys `editor-font-substitution-*` in `editor.ftl:45–48` | Add keys for the new chip/cards |
| Blitz-clean | ✅ no fixed/shadow/custom-props | Maintain |

Download links are a **hardcoded per-font** map in the component
(`editor_inner.rs`); the redesign should keep the link source but move
presentation to the card's action slot.

---

## 7. Renderer switch — current state (M2 baseline)

- **Enums:** `ViewMode { Paginated, Reflow }` (`document_view.rs:32–41`);
  `RenderMode { Paginated, Reflow { available_width_pt } }`
  (`render_layout.rs:40–50`); `LayoutMode { Paginated, Pageless, Reflow {…} }`
  (`loki-layout/src/mode.rs:15–33`).
- **Decision (R-7):** `editor_inner.rs:635–643` — `width < REFLOW_BREAKPOINT_PX
  (900)` → `Reflow`, else `Paginated`; frozen once the user toggles
  (`view_mode_user_set`). **Width-only**: ignores zoom, DPI, and the document's
  own page width/margins. Android-CPU builds hard-pin `Reflow`.
- **Hysteresis (R-8):** there is a 0.5 pt tolerance on *reflow width* changes
  (`RenderMode::matches`, `render_layout.rs:52–69`) so sub-pixel jitter doesn't
  relayout — but **no dead-band around the 900 px mode boundary**, so a window
  dragged across it thrashes between renderers. D2 requires the dead-band.
- **D2 target:** compute page-fit from `Viewport` (width + zoom + DPI + page
  geometry) — which means R-2 (put zoom/DPI on/through `Viewport`) is a
  prerequisite for M2.

---

## 8. Non-paginated typography — current state (M4 baseline)

- **GPU reflow path** (the real layout engine, what most users see): content
  width = `viewport − 2×REFLOW_PADDING_PT` (≈ viewport − 48 CSS px), **no
  max-width cap** → an unbounded measure that runs the full width and reads
  **cramped** on small screens. Confirms Spec 03 §8. (`render_layout.rs:28–30`,
  `loki-layout/src/mode.rs:24–32`.) **R-6.**
- **Android-CPU HTML fallback** (low-fidelity, no caret): already caps at
  `max-width: 820px; margin: 0 auto` (`reflow_view.rs:174–176`) — so the
  *fallback* is bounded but the *primary* path is not. The fix belongs on the
  GPU/layout path.
- M4 wants: bounded measure (~45–75 ch), tuned vertical rhythm, a
  `Breakpoint`-driven type scale, and min/max-bounded reflow.

---

## 9. Where the breakpoint system should live (Spec 03 §5.3 / ADR 0009)

- `Viewport` is currently app-layer (`loki-text`). The `Breakpoint`
  classification and responsive context are **shared UI infrastructure** so
  Presentation/Spreadsheet inherit them.
- **Recommendation (for triage):** place the `Breakpoint` enum + the
  `viewport → Breakpoint` derivation **in `appthere-ui`, alongside the design
  tokens** (`appthere-ui/src/tokens/layout.rs` already holds
  `BREAKPOINT_DESKTOP_PX`, and `appthere-ui` is the shared, downhill UI crate
  after the Spec 01 uphill-edge fix). Relocating `Viewport` itself into
  `appthere-ui` (or a foundation crate it re-exports) lets the three apps share
  one width source. Final placement follows ADR 0009's layer map — flag if the
  dependency-direction gate (`scripts/check-dependency-direction.py`) objects.
- The classification must carry **no Text-specific assumptions** (Spec 03 §5.3).

---

## 10. Milestone readiness

| Milestone | Prereqs present? | Blockers / first step |
|---|---|---|
| **M1 — Breakpoint foundation** | ✅ **Implemented** | Triaged → built: `Breakpoint` (Compact/Medium/Expanded @ 600/1024) + the relocated, zoom/DPI-extended `Viewport` live in `appthere-ui::responsive`; `AtResponsiveContext` + `use_breakpoint`/`use_viewport` expose it; the editor funnels its one measured width into it (no second source). 11 window-free unit tests (R-2, R-3, R-4, R-5 resolved). |
| **M2 — Page-fit switch** | Enums exist (✅); `Viewport` width (✅) | **Needs R-2 first** (zoom/DPI/page-geometry on `Viewport`); add hysteresis dead-band (R-8); replace the 900 guess (R-7). |
| **M3 — Font-warning redesign** | Data + i18n + dismiss (✅) | Add severity signal (R-11); status-bar recovery (R-12); compact chip + Compact card stack (R-9, R-10). |
| **M4 — Non-paginated typography** | Reflow path exists (✅) | Bound the GPU measure + responsive scale (R-6); reuse the 820 px precedent from the HTML path. |
| **M5 — Cross-UI sweep** | Inventory done (this report) | Fix R-13* surfaces; ribbon strip touch target (R-14); verify panel touch heights (R-15). Ribbon collapse stays Spec 04. |

---

## 11. Finding register

| ID | Severity | Finding | Anchor |
|---|---|---|---|
| R-1 | Info | Blitz: no `@media`/`fixed`/`box-shadow`/`var(--` in scope → programmatic breakpoint is the only viable source of truth (confirms D1) | §2 |
| R-2 | ~~High~~ **Resolved (M1)** | `Viewport` now carries `zoom` + `dpi` (default 1.0 / 96); M2 page-fit will populate them. | §3 |
| R-3 | ~~High~~ **Resolved (M1)** | `Viewport` + `Breakpoint` relocated to `appthere-ui::responsive` (shared infra); `loki-text` re-exports `Viewport` for path stability. | §3, §9 |
| R-4 | ~~High~~ **Resolved (M1)** | One `Breakpoint` (600/1024). `BREAKPOINT_DESKTOP_PX = 768` deprecated to `AtHomeTab` only (M5 reconciles); `900` stays the renderer threshold until M2's page-fit replaces it. | §4 |
| R-5 | Info | Spec 01 M4 *width* unification landed; `LayoutContext` never created (not a blocker) | §3 |
| R-6 | High | GPU reflow measure is unbounded → "cramped" readability bug; HTML path already caps at 820 px | §8 |
| R-7 | High | Renderer switch is a width-only guess (900), ignores zoom/DPI/page-fit; comment cites 816 px it doesn't use | §7 |
| R-8 | Med | No hysteresis on the paginated↔reflow boundary → thrash on resize | §7 |
| R-9 | High | Font warning is a full-width band that wraps tall when narrow (named offender) | §6 |
| R-10 | Med | Font warning never stacks vertically / has no compact default | §6 |
| R-11 | Med | No severity model distinguishing metric-compatible vs material substitutions | §6 |
| R-12 | Med | Font warning is dismissible but **not recoverable** (no status-bar indicator) | §6 |
| R-13 | Med | Chrome sweep: title bar + status bar SPILL/WASTE; metadata-panel fixed 140 px label; select fixed 180 px | §5 |
| R-14 | Med | Ribbon tab strip 36 px height < 44 px touch minimum (documented TODO) | §5 |
| R-15 | Low | Spell/metadata/language panel action-button touch heights unverified | §5 |

---

## 12. Open questions for maintainer triage

1. **Breakpoint tiers.** Adopt Spec 03's 600/1024, or reconcile with the
   existing 768 (home tab) and 900 (renderer)? The renderer's 900 is really a
   page-fit proxy and should be *replaced* by M2's computation, not folded into a
   tier — confirm.
2. **Crate placement (R-3).** Put `Breakpoint` (and relocate `Viewport`) into
   `appthere-ui` alongside tokens, or introduce a dedicated foundation crate the
   three apps + `appthere-ui` share? Either must satisfy
   `check-dependency-direction.py`.
3. **`Viewport` extension (R-2).** Add `zoom` and `dpi` (and a page-geometry
   accessor) to `Viewport` now (needed for M2), even though Spec 01 deferred
   them — confirm this is the intended Spec 03 home for zoom.
4. **Sequencing.** M1 → M2 (M2 depends on R-2) → M3/M4 (independent, parallel) →
   M5 sweep. Run audit-first per finding like Spec 01/02, or batch the
   foundation (M1+M2) first?
5. **Testability.** Mirror Spec 02's harness style — pure `viewport.width →
   Breakpoint` and page-fit unit tests with no window — for the M1/M2 acceptance
   criteria?

No code has been changed. Awaiting triage before implementing M1.
