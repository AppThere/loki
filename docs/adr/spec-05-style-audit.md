<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 05 — Style Management Panel: Audit Report

| | |
|---|---|
| **Status** | Audit complete; **no code changes**. Triaged build order below — M1 (provenance resolution) is the keystone everything reads. |
| **Method** | Audit-first per Spec 05 §4: inventory the current panel, the style model + resolution logic, and the six-family modelling completeness before building. |
| **Companion** | [spec-05-style-management-panel.md](spec-05-style-management-panel.md) (the design spec) |
| **Precedent** | Same audit-then-triage flow as [spec-01](spec-01-audit-report.md) / [spec-03](spec-03-responsive-audit.md) / [spec-04](spec-04-ribbon-audit.md). |

This report establishes ground truth and a finding register (**SM-1 … SM-14**). It
makes **no code changes** — implementation waits for triage. The headline
deliverables are the **resolution-gap analysis** (§3), the **six-family
completeness table** (§5), and **one spec-premise correction** (SM-11).

---

## 1. Executive summary

- **The resolution layer is half-built — and the panel ignores the half that exists.** `StyleCatalog::resolve_para` already walks the parent chain and merges properties, cycle-guarded by `MAX_STYLE_CHAIN_DEPTH = 32` (SM-1). But it returns **bare values with no provenance**, and the current panel doesn't even call it — `style_to_draft` reads only the style's *own local* fields (`unwrap_or_default`), which is exactly the "local-only blindness" the spec names (SM-3). So Spec 05's headline (D2, provenance inspector) is a **new provenance-aware resolution layer** (M1), not a from-scratch walk.
- **`resolve_char` is misnamed and there is no character-style resolver.** It resolves `ParagraphStyle.char_props`, not a standalone `CharacterStyle`'s parent chain (SM-2). The character family (a v1 "build-first" family) has **no working resolution today**.
- **The conditional-panel pattern (D1) is already proven viable — low-risk.** `FontWarning` is a `#[component]` that calls the resilient `use_breakpoint()` (defaults to `Expanded`) successfully; the current style/metadata/language panels are plain fns called inside `if` with early returns and so *can't* (SM-5, SM-6). Converting panels to components is mechanical, and `editor_inner.rs` is **849 lines vs its 878 baseline** — D1 specifically avoids the flag-threading that would grow it.
- **The six families are unevenly modelled (SM-9).** Paragraph & character: complete. List/numbering: complete per-level but **no inheritance between list styles** (so the tree view §7 is N/A for lists). **Table: no conditional regions** (header/banding/first-last) — model extension required. **Page: not in the catalog at all** — page setup is per-section `PageLayout`; the §5 OOXML/ODF asymmetry is real and M1 must decide it. Linked: a **one-way** `linked_char_style` field, no reciprocal, no dedicated surface.
- **Spec-premise correction (SM-11):** the spec (§6, §8) repeatedly assumes "existing `COMPAT(i18n)` annotations on internal match keys like 'Default Paragraph Style'." **These annotations do not exist.** `"Default Paragraph Style"` is an *unannotated hardcoded magic string* in `loro_mutation/style.rs`. Built-in-style identification must be defined (likely via the existing `is_default` flag + a known-id set), and the magic string reconciled with the no-hardcoded-strings convention.
- **Staged-Apply and the CRDT snapshot are a good fit (SM-7, SM-8).** Edits stage in a `StyleDraft` and commit on Apply via `write_document_styles` → `loro.commit()` (undoable). The whole catalog is one JSON snapshot, so impact-preview/propagation (§7) is an **in-memory computation over the catalog tree**, and Apply is one atomic, undoable write — matches D4/D5 cleanly.

**Readiness verdict:** the model is a **solid foundation with four concrete gaps** that gate the inspector — (a) provenance, (b) a real character-style resolver, (c) table conditional regions, (d) page styling representation — plus the built-in-style/`COMPAT(i18n)` reconciliation. None are blockers; all are well-scoped model work that must land **before** the family panels that depend on them.

---

## 2. Finding register

| ID | Finding | Evidence |
|---|---|---|
| **SM-1** | `resolve_para` walks the parent chain and merges (`merged_with_parent`, child-wins), cycle-guarded by `MAX_STYLE_CHAIN_DEPTH = 32` — but returns a bare `ResolvedParaProps = ParaProps` with **no provenance**. | `loki-doc-model/src/style/catalog.rs:134-146`, `:22-28`, `:58-65`; `style/props/para_props.rs:213-255` |
| **SM-2** | `resolve_char` resolves `ParagraphStyle.char_props`, **not** a standalone `CharacterStyle`. No `resolve_char_style` exists; the character family has no working parent-chain resolution. | `loki-doc-model/src/style/catalog.rs:157-169` |
| **SM-3** | The panel reads only the style's **own local** fields via `style_to_draft` (`unwrap_or_default`), never calling the existing `resolve_*`. This *is* the "local-only blindness." | `loki-text/.../editor_style_editor/draft.rs:27-77`, esp. `:50-52` |
| **SM-4** | The panel is **paragraph-only**; character/list/table styles are modelled but not editable, and there is no family picker. | `editor_style_editor/mod.rs`, `draft.rs:27` (`style_to_draft(&ParagraphStyle)`) |
| **SM-5** | Style panel mounts as a **plain fn inside `if`** with an early `return rsx!{}`; same for metadata (R-13g) and language panels. None read `use_breakpoint`; no `compact` flag is threaded. | `editor_inner.rs:689-702`, `editor_style_editor/mod.rs:59-62`; `editor_metadata_panel.rs:40,46-49` |
| **SM-6** | D1 is proven viable: `FontWarning` is a `#[component]` calling `use_breakpoint()`; the hook is resilient (`try_consume_context` → `Expanded`). | `editor_font_warning.rs:200-207`; `appthere-ui/src/responsive/mod.rs:93-98` |
| **SM-7** | Staged-Apply confirmed: `StyleDraft` buffer → Apply → `commit_style_to_loro` → `write_document_styles` → `loro.commit()` (undoable). Matches §12 — keep it. | `editor_style_editor/form.rs:213-240`; `editor_style_catalog.rs:20-35` |
| **SM-8** | The whole `StyleCatalog` round-trips as **one JSON snapshot** under `KEY_STYLE_CATALOG_JSON` — atomic, lossless, undoable. Propagation/impact-preview is therefore an in-memory catalog computation. | `loki-doc-model/src/loro_bridge/styles.rs:4-16,24-66` |
| **SM-9** | Six-family modelling is uneven (full table in §5): paragraph/character complete; list complete-but-non-inheriting; **table lacks conditional regions**; **page absent from catalog**; linked one-way. | per §5 table |
| **SM-10** | Defaults: only `default_paragraph_style: Option<StyleId>` + `effective_paragraph_style` fallback. **No** char/table/list/page family defaults and **no** format-default representation. | `catalog.rs:92-102,118-123` |
| **SM-11** | **Spec-premise correction:** no `COMPAT(i18n)` annotations exist anywhere. `"Default Paragraph Style"` is an unannotated hardcoded magic string. `is_default: bool` exists on `ParagraphStyle` and is the real built-in marker. | grep (zero `COMPAT(i18n)`); `loro_mutation/style.rs:20,43,62`; `para_style.rs:57-59` |
| **SM-12** | Re-parenting (M4) needs an explicit **cycle check**: today only the depth cap (32) prevents infinite loops on corrupt input; there is no guard that rejects forming a cycle. | `catalog.rs:22-28` (cap only) |
| **SM-13** | Compact substrate is present: `TOUCH_MIN = 44.0`, `BREAKPOINT_COMPACT_MAX_PX = 600.0`, `position: absolute`-in-`relative` precedent (spell panel); no `fixed`/`box-shadow`/custom-props. | `tokens/spacing.rs:58`; `tokens/layout.rs:80`; `editor_spell_panel.rs`; `CLAUDE.md:309-322,334-335` |
| **SM-14** | Decomposition pressure: current panel = 6 files / ~1212 lines; `editor_style.rs` is **baselined at 316** (over ceiling) and `editor_inner.rs` at **878** (file now 849). New panels must be ≤300, component-per-panel; do not grow either baselined file. | `wc -l`; `scripts/file-ceiling-baseline.txt:15,35` |

---

## 3. Resolution model — gap analysis (the M1 keystone)

**What exists.** A single-parent walk + child-wins merge, cycle-bounded:

```
resolve_para(id):  props = style.para_props
                   for ≤32 ancestors: props = props.merged_with_parent(ancestor.para_props)
                   → ResolvedParaProps (= ParaProps)            // catalog.rs:134-146
```

`merged_with_parent` fills only `None` fields from the parent (child wins), per
property (`para_props.rs:213-255`, `char_props.rs:216-259`). `ResolvedParaProps`
/ `ResolvedCharProps` are **bare type aliases** — the result is indistinguishable
from a hand-set style (`catalog.rs:58-65`).

**What Spec 05 §5 needs (and is missing):**

1. **Provenance.** Every resolved property must carry `(provenance, value)` where
   provenance ∈ {`Local`, `Inherited from ⟨id⟩`, `Default`, `FormatDefault`}.
   Today: values only (SM-1). → **New** resolution result type; the existing
   merge can be instrumented to record *which* ancestor first set each field
   rather than collapsing them.
2. **A real character-style resolver.** `resolve_char` is the paragraph's
   char-props walk, not `CharacterStyle`'s own chain (SM-2). → **New**
   `resolve_char_style(id)` over `character_styles`.
3. **Four-level fall-through.** `Default` (family-default style / `docDefaults`)
   and `FormatDefault` (engine fallback) are not represented (SM-10). → define a
   per-family default-style reference and a format-default source.
4. **Cycle-safe re-parenting** (M4): single-parent + an explicit reject-on-cycle
   check, not just the depth cap (SM-12).

**Page-style asymmetry (Spec 05 §5 — must be decided in M1's ADR).** Page setup
is **not** a named catalog entry; it lives per-section in `PageLayout`
(`layout/page.rs:136-173`, `layout/section.rs`), mirroring OOXML `w:sectPr`. ODF
*does* have named page styles (`style:page-layout` + `style:master-page`). The
M1 ADR must choose: **(A)** add `page_styles` to `StyleCatalog` (ODF-native named
page styles, mapped to OOXML sections on export — the spec's lean), or **(B)**
present each section's `PageLayout` as a synthetic "page style" in the panel
without a catalog change. Recommendation: **(A)**, because the inspector/tree
pattern assumes named, inheritable entries — but flag that page styles do **not**
inherit like the other families (so, like lists, the tree view degrades).

---

## 4. The conditional-panel pattern (Spec 05 §10 / D1) — confirmed ready

The blocker Spec 03 deferred (R-13g) is real and reproduced here (SM-5): a panel
that is a *plain fn called inside `if`* with `None => return rsx!{}` cannot host
a hook, so it cannot call `use_breakpoint()`; threading a `compact` flag instead
pushed `editor_inner.rs` over its ceiling.

The fix is already demonstrated in-tree (SM-6): `FontWarning` is a `#[component]`
that calls `use_breakpoint().is_compact()` directly, and `use_breakpoint` is
resilient (`try_consume_context` → `Breakpoint::Expanded`, never panics). So D1 —
"conditionally-shown panels are components, mounted at the boundary via
`{open().then(|| rsx!{ Panel { .. } })}`" — is a **mechanical, low-risk
conversion**, not new infrastructure. `editor_inner.rs` at 849/878 has headroom,
and D1 keeps it from growing (no flag threading). This pattern should be written
down as a project convention (M3) and the inspector + first family panel born as
components.

---

## 5. Six-family modelling completeness

| Family | Struct (evidence) | Property set | Inheritance | Gap vs Spec 05 |
|---|---|---|---|---|
| **Paragraph** | `ParagraphStyle` (`para_style.rs:22-67`) | `para_props` (28+) + `char_props` | ✓ single-parent (`parent`) | None — *build first* |
| **Character** | `CharacterStyle` (`char_style.rs:20-41`) | `char_props` (23+) | ✓ field present | **No resolver** (SM-2) — *build first* |
| **Linked** | field `linked_char_style` on `ParagraphStyle` (`para_style.rs:37-40`) | — | one-way para→char | No reciprocal, no dedicated surface (§9) |
| **List / Numbering** | `ListStyle` + `ListLevel` (`list_style.rs:137-172`) | per-level (≤9): kind, indents, label align, char_props | **none** (lists independent) | Tree view N/A; inspector applies *per level* |
| **Table** | `TableStyle` / `TableProps` (`table_style.rs:49-92`) | width, align, padding, spacing, border, bg | ✓ `parent` | **No conditional regions** (header/banding/first-last) — model extension |
| **Page** | `PageLayout` per-section (`layout/page.rs:136-173`); **not in `StyleCatalog`** (`catalog.rs:79-103`) | geometry, margins, orientation, columns, headers/footers | n/a | **Absent from catalog**; §5 asymmetry decision (M1) |

Implications for the uniform inspector (§6/§9):

- **Lists** reuse the inspector **per level**, but have no parent tree (§7 degrades to a level list, not a hierarchy).
- **Tables** need `TableProps` extended with conditional-region slots before the table panel's "conditional-format" rows (§9) have anything to bind to.
- **Page** needs the M1 representation decision before its panel exists; expect ODF-native named page styles mapped to OOXML sections on export.
- **Linked** is a *relationship*, not inheritance: the panel surfaces one editing surface over `ParagraphStyle` + its `linked_char_style`; a reciprocal pointer is optional UI sugar, not required by the model.

---

## 6. Apply model & CRDT (Spec 05 §12) — keep as-is

Staged-Apply is confirmed and adequate (SM-7): inputs mutate an in-memory
`StyleDraft`; **Apply** converts it to a `ParagraphStyle`, inserts it into a
cloned catalog, and calls `write_document_styles` → `loro.commit()`, making the
edit a single undoable transaction. Because the catalog is one JSON snapshot
(SM-8), **propagation and impact-preview (§7/D4) are pure in-memory computations
over the resolved tree** — no CRDT-structure work — and Apply remains one atomic
write. No change to the apply model is warranted; the new work is the **staged
multi-override** display (pending vs committed) and computing the **dependent set**
for the impact preview from the catalog.

---

## 7. Compact / decomposition readiness (Spec 05 §11)

The substrate is in place (SM-13): `TOUCH_MIN = 44.0`, `BREAKPOINT_COMPACT_MAX_PX
= 600.0`, and the `position: absolute`-in-`position: relative` precedent
(`editor_spell_panel.rs`) for a bottom-sheet that obeys Blitz limits (no `fixed`,
no `box-shadow`, no custom properties). Decomposition is a hard requirement
(SM-14): the current panel is already 6 files/~1212 lines with `editor_style.rs`
**over the ceiling** (baselined 316). Every new surface — inspector framework,
each family panel, the tree view, the reusable property-row — is its own
component/file ≤300 lines (which *is* the D1 pattern), and neither `editor_style.rs`
nor `editor_inner.rs` (baseline 878) may grow.

---

## 8. Triaged build order (maps Spec 05 §4 / M1–M7)

Model gaps gate the UI, so the sequence front-loads model work:

1. **M1 — Provenance resolution layer + page-style ADR.** ✅ **Core shipped.**
   `style/resolve.rs`: `Provenance` (Local / Inherited(id) / Default /
   FormatDefault) + generic `Resolved<T>`; getter-based `resolve_para_chain`
   (serves para props *and* a paragraph style's run-default char props) and
   `resolve_char_chain` (the standalone `CharacterStyle` resolver SM-2 lacked);
   cycle/depth-guarded, with `para_ancestors` + `para_reparent_cycles` for the
   §7 re-parent guard. 13 tests. The page-style asymmetry + provenance model are
   recorded in [ADR-0012](0012-style-resolution-and-page-styles.md) (page styling
   = ODF-native named page styles in the catalog, mapped to OOXML sections on
   export, a **non-inheriting** family). *Remaining M1 follow-ups (sequenced, not
   blockers): per-family `Default` sources beyond paragraph (char/table defaults),
   and the `page_styles` catalog field itself (lands with the M6 page panel).*
   *Foundation — everything reads it.*
2. **M3 — Conditional-panel pattern.** ✅ **Shipped.** Convention recorded in
   [ADR-0013](0013-conditional-panels-are-components.md) + a CLAUDE.md note:
   conditionally-shown panels are `#[component]`s mounted at the boundary
   (`{open().then(|| rsx!{ … })}`), reading `use_breakpoint()` directly — no
   `compact` flag threaded, so the parent does not grow. Reusable
   `appthere_ui::AtPanelHost` realizes it (a component reading the breakpoint;
   pure unit-tested `PanelPosture::for_breakpoint` → Compact full-width sheet vs
   Medium/Expanded bounded side panel; close control ≥ `TOUCH_MIN`; token
   border/background, no `box-shadow`, in-flow, per Blitz limits). Spec 05's
   family panels (M2/M6) mount their inspector inside it, born responsive.
   *(R-13g's metadata-panel conversion remains Spec 03's milestone — this only
   provides the pattern + host it needs.)*
3. **M2 — Inspector** (paragraph + character) over M1, with provenance rows,
   reset-to-inherited, edit-creates-override, jump-to-ancestor, staged display,
   display-name-over-id. 🚧 **Row model shipped** (`style_inspector.rs`):
   `paragraph_inspector_rows(&catalog, &id)` builds one `InspectorRow` per
   *applicable* property (font/size/bold/italic + alignment/indents/spacing/
   line-height), each carrying its resolved `value_display` and a display-ready
   `RowProvenance` (Local / Inherited{ancestor_id, ancestor_display} / Default /
   FormatDefault) — every property appears regardless of local-set state (the
   local-only blindness fix), and inherited rows name the source ancestor by
   **display name** while keeping its `StyleId` for jump-to-ancestor. Pure +
   i18n-free; 7 tests. **Read-only inspector view shipped**
   (`editor_style_editor/provenance.rs`): a `StyleProvenanceList` `#[component]`
   (ADR-0013) renders every property with its resolved value and a localized
   provenance line (Local / Inherited · ⟨ancestor⟩ / Default / Auto) as a column
   in the style editor panel — the local-only blindness is now *visibly* gone;
   local rows are emphasised. New `style` i18n domain. **Reset-to-inherited
   shipped**: pure `clear_local_property(&mut style, property)` (10 tests total)
   + a per-row reset control on locally-set rows that clears the override via the
   existing `commit_style_to_loro` path (undoable) and re-derives the draft, so
   the property falls through to its inherited/default/engine value. *Remaining
   M2: edit-creates-override staged display (the §12 staged-vs-committed model —
   the form already writes overrides live on Apply), jump-to-ancestor navigation,
   promoting the panel into `AtPanelHost`, and the built-in-style identification
   question (SM-11 — `is_default` + a known-id set; decide whether to add the
   `COMPAT(i18n)` annotations the spec assumed or drop that framing).*
4. **M5 — Creation & management** (create → pick parent → override; rename;
   re-parent; delete-with-orphan-handling; built-in rules).
5. **M4 — Tree view + propagation + impact preview** (in-memory over the catalog;
   cycle-guarded re-parent from M1).
6. **M6 — Remaining families.** Linked (relationship surface), list (per-level),
   then **table** (after `TableProps` conditional-region extension) and **page**
   (after the M1 representation decision).
7. **M7 — Compact posture** applied throughout (bottom-sheet, breadcrumb tree,
   segmented family nav), verified at <600.

**Prerequisite model extensions** (call out so they're not discovered late):
provenance result type + char-style resolver + per-family defaults (M1);
`TableProps` conditional regions (before table panel, M6); page-style catalog
representation (M1 decision, realized before page panel, M6).

---

## 9. What this audit deliberately did **not** do

- No code changes (per Spec 05's audit-first method and the spec-04 precedent).
- Did not re-verify style round-trip fidelity (→ Spec 02).
- Did not touch Spec 03's R-13g closure (this audit only confirms the pattern
  that unblocks it).
- Did not design property-by-property panel layouts — that is M2/M6 work once the
  M1 resolution shape is fixed.
