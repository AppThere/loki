<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 05 — Style Management Panel

| | |
|---|---|
| **Status** | Draft — pending implementation |
| **Scope** | AppThere Loki (Text); the inspector framework is shared UI infrastructure where reusable |
| **Sequence** | 5 of 6 — depends on a trusted style model, the responsive foundation, and the Write→Styles entry point |
| **Depends on** | Spec 02 (trusted style round-trip), Spec 03 (breakpoint context — **this spec resolves the deferred conditional-panel context problem**), Spec 04 (Write→Styles entry point), Spec 01 (ADR 0009 layering, token location) |
| **Feeds** | A future direct-formatting-cleanup tool (out of scope here); Spec 06 measures resolution cost |

---

## 1. Context & Motivation

Loki's philosophy is **named styles over direct formatting** — reusable paragraph, character, and other styles are the intended way to format a document. But the panel that manages them is a rough prototype: mostly unstyled controls in a panel, and — critically — it **only shows properties that are locally set on the style**. Everything a style *inherits* from its parent is invisible. That defeats the entire point of an inheritance-based style system: you can't see where a property actually comes from, so you can't reason about it or change it in the right place.

This spec rebuilds the panel into a real style-management surface that competes with LibreOffice and Word, organized around two ideas:

1. **A resolved-vs-overridden inspector.** For every property of a style, show its resolved value *and its provenance* — is it set locally on this style, inherited from a named ancestor, or a document/format default? One click resets a local override back to inherited.
2. **An inheritance tree view.** See the whole style hierarchy, understand what depends on what, and change a base style with confidence that the change propagates to its dependents — with an impact preview before you commit.

It covers **all six style families** (paragraph, character, linked, list/numbering, table, page), unified by the same inspector pattern, and it must be usable on a phone.

It also pays down an architectural debt: Spec 03 deferred a fix because **conditionally-rendered panels couldn't read the responsive context**. The styling panel is *entirely* conditionally-mounted family panels, so this spec **owns and defines the sanctioned pattern** for that — once, here, instead of per panel.

This is **audit-first**: inspect the current panel, the style model and its resolution logic, and how completely each of the six families is modeled, before building. References below are illustrative.

---

## 2. Relationship to Prior Specs (read first)

- **Spec 02 — trust.** The inspector is only as correct as the style model underneath it. Spec 02's round-trip axis is what guarantees styles survive import/export without silent loss (cf. the collapsed-style risk it names). This spec assumes that trust; it does not re-verify the model.
- **Spec 03 — the deferred problem is ours.** `use_breakpoint()` is resilient (defaults to `Breakpoint::Expanded` via `try_consume_context`). Spec 03 deferred R-13g because a panel rendered as a *plain function inside an `if`* with an early return can't host a hook. The styling panel can't dodge this — it's all conditional panels. §10 defines the fix and that fix retroactively unblocks R-13g.
- **Spec 04 — the entry point.** The Write tab's **Styles group** is how this panel is opened. Style application (clicking a style to apply it) lives in the ribbon/quick-styles; *this* panel is for **defining and managing** styles.
- **ADR 0009** governs layering; **design tokens** come from their post-Spec-01 location. **Blitz constraints** bind: no `position: fixed`, no `box-shadow`, no CSS custom properties.

---

## 3. Goals / Non-Goals

**Goals**

- A resolved-vs-overridden inspector with per-property provenance and one-click reset-to-inherited.
- An inheritance tree view with propagation, impact preview, and re-parenting.
- First-class **style creation** (pick family, pick parent, name, override) and management.
- Panels for all six families, sharing the inspector framework.
- A documented, sanctioned **conditional-panel context pattern** (§10).
- Full Compact/phone usability.

**Non-Goals**

- "Update style to match selection" (Loki discourages direct formatting; the control isn't useful here).
- The direct-formatting → named-style **cleanup tool** (explicitly deferred to a future spec).
- Adding style *properties the renderer can't honor* — the panel edits what the model+renderer support, audited like Spec 04's Insert gate.
- Re-verifying style round-trip (→ Spec 02).
- Style *application* UX (→ Spec 04's quick-styles); this panel is definition/management.

---

## 4. Working Method

1. **Inventory** the current panel and the style model: how styles, parents, defaults, and the six families are represented; where resolution currently happens (if at all).
2. **Build the resolution layer** (§5) — provenance-aware style resolution is the foundation everything else reads.
3. **Build the inspector** (§6), then the tree view (§7), then creation/management (§8).
4. **Establish the conditional-panel pattern** (§10) before building the family panels, so every family panel is born correct.
5. **Implement the six family panels** (§9), paragraph+character first (they exercise the full pattern), then the rest.
6. **Apply Compact posture** (§11) throughout.

Standing standards apply; the panels are substantial, so **decomposition under the 300-line ceiling is a first-class requirement** (§11), not an afterthought.

---

## 5. Inheritance & Resolution Model

The technical core. Style inheritance in both formats is **single-parent** (OOXML `w:basedOn`, ODF `style:parent-style-name`), so the hierarchy per family is a **tree**, not an arbitrary graph — rooted at the family's default style. (Linked styles add a paragraph↔character cross-reference, which is *not* inheritance and is handled separately in §9.)

For a style `S` and property `P`, resolution yields a value **and its provenance**:

1. **Local** — `P` is set on `S` itself → `(Local, value)`.
2. **Inherited** — walk `S`'s parent chain to the first ancestor `A` that sets `P` → `(Inherited from A, value)`.
3. **Default** — fall to the family default style / document defaults (`docDefaults` in OOXML) → `(Default, value)`.
4. **Format default** — the format/application fallback → `(FormatDefault, value)`.

Every property the inspector shows carries this `(provenance, value)`. "Reset to inherited" removes the local override so the property falls through to its inherited value. Editing a property adds/updates a local override. This resolution is also what Spec 06 will measure, since it runs on every inspector open and every dependent recompute.

**The OOXML/ODF page-style asymmetry is a real design decision, not a detail.** ODF has named page styles (`style:page-layout` + `style:master-page`); OOXML has no named page-style equivalent — page setup lives in section properties (`sectPr`). The audit must decide how the unified model represents page styling across both, and how the page-family panel presents it (likely ODF-native named page styles, mapped to OOXML sections on export). Flag this explicitly in the resolution-model ADR.

---

## 6. The Resolved-vs-Overridden Inspector

The headline surface. For the selected style, list **every** applicable property — not just locally-set ones (the current panel's central failing) — each row showing:

- the property and its **resolved value**;
- a clear **provenance indicator**: Local · Inherited from *⟨ancestor name⟩* · Default;
- for inherited/default rows, the edit affordance creates a local override;
- for local rows, a one-click **reset-to-inherited** that removes the override.

Provenance must be glanceable — a user should see at a glance which properties this style actually *owns* versus passively receives. Inherited rows should let the user **jump to the ancestor** that sets the property (this is the "identify where a property is set, change it for all dependents" workflow). Display uses each style's localized **display name**, while all operations key on the stable internal **style id** (respecting the existing `COMPAT(i18n)` annotations on internal match keys like "Default Paragraph Style").

Edits are **staged**, not live (§12) — the inspector shows pending overrides distinctly from committed ones until Apply.

---

## 7. The Inheritance Tree View

A view of the family's inheritance tree, for understanding and managing structure:

- **Visualize** the parent→child hierarchy; select a node to inspect it (§6).
- **Propagation is the point.** Changing a property on a base style changes it for every descendant that doesn't locally override it. The tree makes the affected subtree visible.
- **Impact preview before Apply.** When a staged edit to a base style would affect dependents, show *how many* and *which* dependent styles change before the user commits. No surprise cascades.
- **Re-parenting.** Let the user change a style's parent (`basedOn` / `parent-style-name`); the inspector re-resolves and the impact preview shows the consequence. Guard against cycles (single-parent + cycle check keeps the tree a tree).

On Compact, the full tree is impractical; it degrades to a breadcrumb + drill-down (§11), not a shrunken graph.

---

## 8. Style Creation & Management

"Make it easy to create reusable styles" is a primary goal, so creation is first-class:

- **Create:** choose family → choose parent (defaulting sensibly within the family) → name it → the inspector opens showing everything inherited from the parent, ready to override selectively. Creating a style is "pick a parent and override what differs," which is the inheritance model working as intended.
- **Manage:** rename (display name), re-parent (§7), delete (with dependent-impact warning — deleting a parent must handle its orphans predictably, e.g. re-parent to grandparent).
- **Built-in vs. user styles:** built-in/default styles (the `COMPAT(i18n)` internal-keyed ones) may restrict deletion/rename; the audit determines the rules and the panel enforces them.

---

## 9. The Six Family Panels

All six share the §6 inspector and §7 tree; they differ only in their **property set** and a few structural wrinkles:

- **Paragraph** — alignment, indents, spacing, line height, borders, shading; references a linked character aspect for run defaults. *(Build first.)*
- **Character** — font family/size/weight/style, color, underline, effects. *(Build first — paragraph+character exercise the whole inspector pattern.)*
- **Linked** — the paragraph↔character cross-reference (OOXML `w:link`): one editing surface governing both aspects. Not inheritance; a distinct relationship the panel makes explicit.
- **List / Numbering** — per-level definitions: numbering format, glyph, indents per level. A more complex, level-indexed surface reusing the inspector per level.
- **Table** — table-wide formatting plus conditional regions (header row, banding, first/last column). The inspector extends to conditional-format slots. Only meaningful because tables render (cf. Spec 04's capability gate).
- **Page** — the ODF/OOXML asymmetry from §5: named page styles (geometry + master page) presented ODF-native, mapped to OOXML sections on export.

Equal depth isn't required in v1, but the **inspector pattern is uniform** across all six. Build order follows §4: framework + paragraph/character first, then linked/list/table/page reusing the proven pattern.

---

## 10. The Conditional-Panel Context Pattern (sanctioned)

**This is the architectural deliverable Spec 03 deferred.** The problem: a panel rendered as a *plain function called inside an `if`*, with an early return, can't host a hook (`use_breakpoint()` included), and threading a `compact` flag down breached the `editor_inner.rs` ceiling.

**Decision (D1): conditionally-shown panels are proper `#[component]`s, not plain functions called inside `if`.** A component owns its own hook scope, which Dioxus manages correctly across mount/unmount — so the component can call the resilient `use_breakpoint()` directly and read responsive context without any prop threading and without growing the parent. The parent mounts it conditionally at the component boundary (`{open().then(|| rsx! { StyleFamilyPanel { .. } })}`); the boundary handles hook scoping.

Consequences:

- Every family panel and sub-panel in this spec is a component from the start — born able to read the breakpoint.
- No `compact` flag is threaded through `editor_inner.rs`; the ceiling pressure that forced the Spec 03 deferral disappears.
- **This pattern retroactively unblocks Spec 03's R-13g** (metadata-panel label stacking): converting that plain-fn panel to a component lets it read the breakpoint the same way. Closing R-13g remains Spec 03's milestone; this spec only establishes the pattern it needs.

The pattern is documented as a project convention so future conditionally-mounted surfaces follow it by default.

---

## 11. Phone / Compact Usability

The panel must be fully usable at Compact (Spec 03's < 600 tier), which means it is not merely a narrowed desktop side panel:

- **Posture:** at Compact the panel becomes a full-surface or bottom-sheet presentation rather than a side-by-side panel (no room beside the document). Within Blitz limits — no `position: fixed` (so the sheet lives in flow / via the patched mechanism, not pinned), no `box-shadow` (elevation via token border/background).
- **Inspector rows** stack and meet `TOUCH_MIN` (44px, Spec 03) touch targets.
- **Tree view** degrades to breadcrumb + drill-down (§7).
- **Family/section navigation** within the panel uses a compact tab or segmented control so all six families and their property groups are reachable without horizontal scrolling.
- **Decomposition under the 300-line ceiling is required**, not incidental: the inspector framework, each family panel, the tree view, and reusable property-row components are separate files/components. This both satisfies the ceiling and *is* the §10 pattern (each panel a component). The audit plans the decomposition before code is written.

---

## 12. Apply Model

Keep the current **staged-edit + Apply** model (the maintainer confirmed it's adequate):

- Property edits in the inspector are **staged**, shown distinctly from committed values.
- **Apply** commits the staged overrides to the model (Loro), which propagates to dependents via resolution — the impact preview (§7) already told the user what that entails.
- **Discard** reverts staged edits. Apply is per-panel batch, not per-property live-write.

Live preview is *not* required; the explicit Apply is the intended model and keeps CRDT writes deliberate.

---

## 13. Key Decisions (ADR-style)

**D1 — Conditional panels are components, not plain fns in `if`.** Fixes the Spec 03 context problem at its root, threads no flags, respects the ceiling, unblocks R-13g. Tradeoff: a small discipline (everything's a component) — which is idiomatic Dioxus anyway.

**D2 — Inspector shows all properties with provenance, not just local ones.** Directly fixes the current panel's central failing; provenance is what makes inheritance reasonable-about. Tradeoff: more to render and resolve per open (Spec 06 measures it) — essential to the feature's purpose.

**D3 — Inheritance is a tree (single-parent); linked styles are a separate cross-reference.** Matches both formats' actual model; cycle-guarded re-parenting keeps it a tree. Tradeoff: page styles don't fit the tree cleanly across formats (§5) — handled as an explicit asymmetry, not forced.

**D4 — Impact preview before Apply.** Changing a base style is powerful; users see the blast radius first. Tradeoff: computing affected dependents on stage — cheap relative to the value of no surprise cascades.

**D5 — Staged Apply, no live preview.** Confirmed-adequate, keeps CRDT writes deliberate. Tradeoff: less immediate than live — accepted; deliberate is correct for shared/named styles.

**D6 — All six families, uniform inspector; paragraph/character first.** Covers all styling needs with one pattern. Tradeoff: list/table/page surfaces are complex — phased after the pattern is proven, equal depth not required in v1.

---

## 14. Milestones & Acceptance Criteria

**M1 — Resolution layer.** Provenance-aware resolution (Local/Inherited-from/Default/FormatDefault) over the single-parent tree; the page-style asymmetry decided in an ADR. *Accept:* given a style and property, the layer returns the correct value *and* provenance, including multi-level inheritance; cycles are impossible.

**M2 — Inspector.** All applicable properties shown with provenance; reset-to-inherited; edit-creates-override; jump-to-ancestor; staged display; display-name UI over id operations. *Accept:* an inherited property is visibly distinguished from a local one, names its source ancestor, and resets cleanly; the current "local-only" blindness is gone.

**M3 — Conditional-panel pattern.** §10 established and documented; the inspector and a first family panel are components reading the breakpoint with no parent prop-threading. *Accept:* a conditionally-mounted panel reads the correct breakpoint at Compact and Expanded without growing `editor_inner.rs`; the pattern is written down as a convention.

**M4 — Tree view + propagation.** Hierarchy visualization, impact preview, cycle-guarded re-parenting. *Accept:* editing a base style's property previews the exact dependent set affected before Apply; re-parenting re-resolves and can't form a cycle.

**M5 — Creation & management.** Create (family→parent→name→override), rename, re-parent, delete-with-orphan-handling; built-in-style rules enforced. *Accept:* a new style created from a parent shows all inherited values ready to override; deleting a parent handles dependents predictably.

**M6 — Six family panels.** Paragraph + character first, then linked/list/table/page on the shared inspector; linked cross-reference explicit; page family per the §5 decision. *Accept:* all six families are editable through the uniform inspector; linked styles govern both aspects; table conditional regions are reachable.

**M7 — Compact usability.** Bottom-sheet/full-surface posture, touch targets, breadcrumb tree, compact navigation, ceiling-compliant decomposition. *Accept:* the full panel — including the tree and all six families — is operable at < 600 width with no horizontal scrolling and 44px targets.

---

## 15. Out of Scope

- "Update style to match selection" (not useful under Loki's no-direct-formatting philosophy).
- The direct-formatting → named-style cleanup tool (deferred to its own future spec).
- Adding style properties the renderer can't honor (audited out, like Spec 04's Insert gate).
- Style *application* / quick-styles UX (→ Spec 04).
- Re-verifying style round-trip fidelity (→ Spec 02).
- Closing Spec 03's R-13g itself (this spec provides the pattern; R-13g remains Spec 03's milestone).
