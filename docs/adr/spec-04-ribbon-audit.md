<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 04 — Ribbon UI Refinement: Audit Report

| | |
|---|---|
| **Status** | Audit complete — **pending maintainer triage** (no implementation yet) |
| **Method** | Audit-first per Spec 04 §4: inventory the ribbon, run the render-capability audit (§10 — the committed capability table), and confirm the Blitz layout surface for the collapse engine. |
| **Companion** | [spec-04-ribbon-ui-refinement.md](spec-04-ribbon-ui-refinement.md) (the design spec) |
| **Precedent** | Same audit-then-triage flow as [spec-01](spec-01-audit-report.md) / [spec-02](spec-02-conformance-inventory.md) / [spec-03](spec-03-responsive-audit.md). |

This report establishes ground truth and a finding register (RB-1 … RB-12). It makes
**no code changes** — implementation waits for triage. The §4 **capability table**
is the headline deliverable.

---

## 1. Executive summary

- **The framework is more built than the spec assumed — the work is mostly *content*, not new framework.** The contextual-tab mechanism already exists (`RibbonTabDesc.is_contextual`, amber styling) but is unused; labeled groups already exist (`AtRibbonGroup.label: Option<String>`) — the Write/Home tab simply passes `None` (icon-only) while Publish passes `Some` (labeled). So **D1 (rename), D2 (labeled everywhere), and the contextual mechanism are largely a matter of using what's there** (RB-1).
- **What's genuinely missing is the collapse engine.** Today the ribbon has only a collapse *toggle* (hide the whole content row) + `overflow-x: auto` scroll — no width-driven full→condensed→overflow cascade (RB-2). This is the real M3 build.
- **The capability table (§4) is decisive:** 5 objects **Create-ready** (images, tables, headers/footers, hyperlinks, **footnotes**), 1 **Render-only** (math), 1 **Unsupported** (shapes). Notably the audit **upgrades footnotes** from the spec's seed ("renders to an extent") to *complete render* → Create-ready (RB-7), and **confirms math render-only / shapes unsupported** with code evidence (RB-8/RB-9).
- **Blitz surface is sufficient:** a component can measure its **own** width via `onmounted`→`get_client_rect().await`, `use_viewport().inner_width_px` gives the px the width-driven engine needs, the overflow menu can use `position: absolute` (spell-panel precedent), and the `resolve_page_fit` hysteresis pattern (`PAGE_FIT_HYSTERESIS_PX = 48`) is the template for collapse hysteresis (RB-10).
- **One spec correction:** the "existing bottom-ribbon-for-touch placement" **does not exist** — only safe-area-inset + Compact-detection infrastructure for it. M6 frames it "where applicable," so it stays optional (RB-11).

**Readiness:** M1 (rename + framework polish), M2 (labeled groups), and M4 (Insert from the table) are low-risk and mostly content. M3 (collapse engine) is the substantive new build. **No code changed.**

---

## 2. Ribbon framework inventory (`appthere-ui/src/components/ribbon/`)

| Component | File | Renders | Notes |
|---|---|---|---|
| `AtRibbon` | `mod.rs:84` | tab strip + content row shell | props: `tabs`, `active_tab`, `on_tab_select`, `collapsed`, `on_toggle_collapse`, `tab_content` |
| `AtRibbonTabStrip` | `tab_strip.rs:31` | tab-label row + collapse button | height **36 px** (`RIBBON_TAB_STRIP_HEIGHT`, `tokens/layout.rs:65`) — **R-14** |
| `AtRibbonContent` | `content_row.rs:26` | scrollable group row | `overflow-x: auto` (the only overflow today) |
| `AtRibbonGroup` | `group.rs:28` | labeled section | `label: Option<String>` — **labeled groups already supported** |
| `AtRibbonIconButton` | `button.rs:38` | 44×44 button | icon child *or* text-span child (labeled) — both styles already possible |
| `AtRibbonSelect` | `select.rs:52` | style-picker dropdown | hardcoded `width: 180px` (`select.rs:73`) — **R-13e** |
| `RibbonTabDesc` | `mod.rs:57` | `{ label, is_contextual, aria_label }` | **contextual mechanism already present** (`is_contextual`) |

**Framework capabilities — present vs. missing:**

| Capability | Status |
|---|---|
| Contextual-tab mechanism (`is_contextual`, amber styling) | ✅ present, **unused** by the app (no selection signal feeds it — RB-5) |
| Labeled groups (`label: Some(..)`) | ✅ present, used only by Publish |
| Labeled *buttons* (text span in `AtRibbonIconButton`, `label_node`) | ✅ present (Publish) |
| Collapse **toggle** (hide content row) + horizontal scroll | ✅ present |
| **Width-driven collapse cascade** (full→condensed→overflow) | ❌ **missing** — the M3 build (RB-2) |
| **Overflow "more" menu** | ❌ missing |
| Per-group **collapse priority** + condensed/overflow representations | ❌ missing |

---

## 3. Tab / group / control inventory

| Tab | Idx | Style | Groups → controls |
|---|---|---|---|
| **Home** (→ Write) | 0 | **icon-only** (`label: None` everywhere) | Document (Save / Save As / Save as Template), History (Undo / Redo), Styles (`AtRibbonSelect` para style — the Spec 05 entry point), Paragraph (edit-style), Inline (Bold / Italic / Underline / Strike / Super / Sub) |
| **Publish** | 1 | **labeled groups** (the target standard) | Export (PDF/X, EPUB — labeled buttons), Metadata (Edit metadata — labeled) |

Tabs are composed in `editor_inner.rs` (the `AtRibbon { tabs: vec![…], tab_content: match … }`); Home content in `editor_ribbon.rs`, Publish in `editor_publish.rs`. i18n keys in `ribbon.ftl` / `publish.ftl`.

**Mapping to the Spec 04 tab set (§9):** Write's existing groups (Document/History/Styles/Paragraph/Inline) map cleanly onto the proposed **Write** (Clipboard/Font/Paragraph/Styles/Editing); the existing controls are a subset — the spec's instruction "map, don't invent" holds. **Insert / Layout / References / Review** are net-new tabs whose *contents* are gated by §4 (Insert) and the existing model/render features (Layout/References/Review map to already-supported page geometry, footnotes, spelling, comments).

---

## 4. The render-capability table (committed deliverable — Spec 04 §10)

For each insertable object: **Import** (mapper parses it), **Render** (layout + paint, evidenced by code + an ACID `TC-*` case + `fidelity-status.md`), **Create** (a model-construction path).

| Object (model type) | Import | Render | Create | **Verdict** | Evidence |
|---|---|---|---|---|---|
| **Image** (`Inline::Image`) | ✅ | ✅ | ✅ cheap | **Create-ready** | import `docx/mapper/images.rs`; paint `loki-vello/src/image.rs:29` `paint_image`; ACID TC-DOCX-023/024. Create = construct `Inline::Image` + existing `loki-file-access` picker. |
| **Table** (`Block::Table`) | ✅ | ✅ | ✅ cheap | **Create-ready** | import `docx/mapper/table.rs:25`; layout `loki-layout/src/flow.rs:1527` `flow_table`; ACID TC-DOCX-003…007. Create = build an n×m `Table`. |
| **Header / Footer** (`PageLayout`) | ✅ | ✅ | ✅ moderate | **Create-ready** | layout `flow.rs:960` `assign_headers_footers` (per-page, PAGE/NUMPAGES live); ACID TC-DOCX-018 / TC-ODT-008; `fidelity-status.md` §1. Create = insert a `HeaderFooter { blocks }` into `PageLayout` (needs a small edit surface, not geometry). |
| **Hyperlink** (`Inline::Link`) | ✅ | ✅ | ✅ cheap | **Create-ready** | import `docx/mapper/inline.rs:85`; renders as styled inline text; ACID TC-DOCX-022. Create = wrap selection in `Inline::Link(target)`; URL via text entry. |
| **Footnote / Endnote** (`Inline::Note`) | ✅ | ✅ **complete** | ✅ cheap | **Create-ready** ⬆ | import `docx/mapper/inline.rs:260/267`; layout `flow.rs:1126` `flow_footnotes` (numbering, per-section restart, separator); ACID TC-DOCX-020 / TC-ODT-009; `fidelity-status.md` §1 row 16. **Audit upgrades the spec's "renders to an extent" → complete render.** Create = insert `Inline::Note(kind, blocks)`; auto-numbers. |
| **Math** (`Inline::Math` / MathML) | ✅ | ✅ **partial** | ❌ needs input surface | **Render-only** | import `docx/mapper/inline.rs:141` (OMML→MathML); layout `loki-layout/src/math/` (`mod.rs`/`compose.rs`/`shape.rs` — fractions, scripts, radicals, fences); ACID TC-DOCX-026. **Known first-pass gaps** (matrices, n-ary, accents, full mathspacing — `fidelity-status.md` §1 row 24). **No create path** — authoring needs an equation-input surface → a **future spec**, not this Insert tab. |
| **Shape** (DrawingML preset/freeform) | ❌ | ❌ | — | **Unsupported** | **No shape-geometry rendering anywhere** in `loki-layout`/`loki-vello` (the `math/shape.rs` is math-delimiter stretching, not DrawingML). DOCX drawings only reach the model as `Inline::Image` anchors. Shape support is a **future renderer spec** before any Insert control. |

**Tally:** **5 Create-ready** · **1 Render-only** (math) · **1 Unsupported** (shapes).

This satisfies M4's acceptance shape: every Insert control maps to a Create-ready row; math and shapes get **no control** (no dead UI); imported docs containing math/shapes still display (math renders; DOCX shapes fall back to image anchors).

---

## 5. Existing create/insert paths (for M4 reuse)

No new model-construction is needed for the Create-ready set — the constructors + Loro mutation layer suffice:

- **Image:** `Inline::Image` + `loki_file_access` picker (already used app-wide). No insert UI yet.
- **Table:** build a `Table` from grid geometry. No insert UI yet.
- **Header/Footer:** `PageLayout.header/footer` hold `Vec<Block>`; mutate via the Loro layer. Needs a small block-editor surface (the style-editor panel is a reusable pattern).
- **Hyperlink:** `Inline::Link(target, display)` over the selection.
- **Footnote/Endnote:** `Inline::Note(kind, blocks)` at the cursor; auto-numbered by `flow_footnotes`.
- Mutation layers: `loki-doc-model/src/loro_mutation/{text,block,style}.rs`.

---

## 6. Blitz layout surface for the collapse engine (Spec 04 §4.3)

| Need | Finding |
|---|---|
| **Measure the ribbon's own width** | ✅ `onmounted` → `MountedData::get_client_rect().await` works on any mounted element (the dioxus-native patch, `patches/dioxus-native-dom/src/mounted.rs`). The collapse engine measures the content row directly, or uses `use_viewport().inner_width_px` as a proxy. |
| **Width-driven (px, not class)** | ✅ `use_viewport()` exposes `inner_width_px: f32` (D3 needs px; `use_breakpoint()`'s class alone is insufficient). |
| **Overflow "more" menu** | ✅ `position: absolute` in a `position: relative` ancestor + `z-index` (the spell panel `editor_spell_panel.rs` is the production precedent). **No** `position: fixed` (collapses to absolute), **no** `box-shadow`, **no** custom properties — elevation via border/background. |
| **Hysteresis** | ✅ mirror `responsive::page_fit::resolve_page_fit` (`PAGE_FIT_HYSTERESIS_PX = 48`): keep the current collapse state until width crosses `threshold ± band`, so resizing across a boundary can't thrash. A `RIBBON_COLLAPSE_HYSTERESIS_PX` token follows the same shape. |

---

## 7. Corrections & confirmations vs. the spec's seeds

- **Footnotes** seeded "renders to an extent" → audit finds **complete render** → **Create-ready** (not render-only). (RB-7)
- **Math** "renders; create likely needs a surface" → **confirmed Render-only**; the create gap is real. (RB-8)
- **Shapes** "render status unknown" → **confirmed Unsupported** (zero shape-geometry render code). (RB-9)
- **"Existing bottom-ribbon-for-touch placement"** → **does not exist**; only safe-area-inset (`app.rs`) + Compact-detection infrastructure. M6 says "where applicable", so it's an optional build, and the collapse cascade may make it unnecessary on phones. (RB-11)
- **Framework** "half-built" → contextual + labeled-group + labeled-button capabilities **already exist**; the missing piece is specifically the **collapse cascade** and a **selection signal** for contextual tabs. (RB-1/RB-2/RB-5)

---

## 8. Milestone readiness

| Milestone | Prereqs present? | First step / blocker |
|---|---|---|
| **M1 — Framework + rename** | ✅ framework, contextual mechanism, i18n | Rename `ribbon-tab-home`→`ribbon-tab-write` (+ keep the Home *screen* key distinct); verify the framework renders sanely at Expanded-by-default (no context). Low-risk. |
| **M2 — Labeled groups everywhere** | ✅ `AtRibbonGroup.label`, `label_node` | Pass `Some(label)` + text buttons in the Write tab (`editor_ribbon.rs`); retire `label: None`/icon-only. Content change. |
| **M3 — Collapse cascade** | ⚠️ needs new engine | Build the width-driven engine: per-group priority + condensed/overflow reps + overflow menu (`position: absolute`) + hysteresis (mirror `page_fit`). **The substantive build.** R-13e select-width handled in *condensed*. |
| **M4 — Render-gate + Insert tab** | ✅ capability table (§4) + create paths (§5) | Add the **Insert** tab with controls for the 5 Create-ready objects only; commit the §4 table. No math/shape controls. |
| **M5 — Remaining tabs + contextual** | ⚠️ needs selection signal (RB-5) | Add Layout/References/Review from existing features; add a `selected_object: Signal<Option<…>>` in `EditorState`, set it from pointer hit-tests, drive Table/Picture contextual tabs via `is_contextual`. |
| **M6 — Touch posture** | ✅ `TOUCH_MIN`, breakpoint | Bump tab strip to `TOUCH_MIN` at Compact (R-14); condensed select sizing (R-13e); bottom-ribbon placement *optional* (RB-11). |

---

## 9. Finding register

| ID | Severity | Finding | Anchor |
|---|---|---|---|
| RB-1 | Info | Framework already supports contextual tabs + labeled groups/buttons — D1/D2/contextual are mostly *content* work | §2, §7 |
| RB-2 | High | No width-driven collapse cascade (only a collapse toggle + `overflow-x: auto`) — the M3 build | §2 |
| RB-3 | Info | R-13e confirmed: `AtRibbonSelect` hardcodes `width: 180px` (`select.rs:73`) — condensed state must size it | §2 |
| RB-4 | Info | R-14 confirmed: tab strip `36px` (`RIBBON_TAB_STRIP_HEIGHT`) < 44 px touch min | §2 |
| RB-5 | Med | No selection-state signal exists; contextual Table/Picture tabs need a new `EditorState` signal fed by hit-tests | §2, §8 |
| RB-6 | Info | Two-Homes collision confirmed: `ribbon-tab-home = Home` (`ribbon.ftl:4`) vs the `Home()` screen (`home.rs:146`) | §3 |
| RB-7 | Info | **Footnotes upgraded** to Create-ready (complete render, simple create) — not render-only | §4 |
| RB-8 | Info | Math is **Render-only** (renders partial; no create path) — equation editor = future spec | §4 |
| RB-9 | Info | Shapes are **Unsupported** (no shape-geometry render) — future renderer spec | §4 |
| RB-10 | Info | Blitz surface sufficient: self-width via `get_client_rect`, px via `use_viewport`, overflow via `position: absolute`, hysteresis via `page_fit` pattern | §6 |
| RB-11 | Info | **Spec correction:** "existing bottom-ribbon-for-touch placement" does not exist (only infrastructure); M6 optional | §7 |
| RB-12 | Info | Publish tab is the visual-standard *source*; audit confirms it stays its own tab (export is a distinct concern) | §3 |

---

## 10. Open questions for maintainer triage

1. **Sequencing.** M1/M2/M4 are low-risk content; M3 (collapse engine) is the real build and M5 needs a new selection signal. Land M1+M2 (rename + labeled standard) first, then M4 (Insert from the table), then M3 (collapse), then M5/M6 — or build the framework collapse engine before the content?
2. **Write tab group set.** Confirm the §9 Write grouping (Clipboard / Font / Paragraph / Styles / Editing) — the inventory has no Clipboard (cut/copy/paste) or Find/Replace controls yet. Map only existing controls (per "don't invent"), or is adding cut/copy/paste + find/replace in scope here?
3. **Header/Footer create surface (M4).** The 4 other Create-ready objects are cheap; headers/footers need a small block-editor surface. In scope for M4, or defer the header/footer Insert control while shipping the other four?
4. **Math & shapes.** Confirm math (Render-only) and shapes (Unsupported) get **no** Insert control and become their own future specs (equation editor; shape renderer) — as the capability table dictates.
5. **Collapse hysteresis + priority.** Reuse `PAGE_FIT_HYSTERESIS_PX = 48` (or a dedicated `RIBBON_COLLAPSE_HYSTERESIS_PX`)? And where do per-group collapse priorities get declared — on `AtRibbonGroup` as a new prop?
6. **Bottom-ribbon-for-touch (RB-11).** Build it now (it doesn't exist), or rely on the collapse cascade at Compact and defer bottom placement?

No code has been changed. Awaiting triage before implementing M1.
