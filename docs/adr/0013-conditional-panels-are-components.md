<!--
SPDX-License-Identifier: Apache-2.0
-->

# ADR-0013: Conditionally-mounted panels are components, not plain functions

**Status:** Accepted
**Date:** 2026-06-30
**Deciders:** AppThere engineering
**Resolves:** [`spec-05-style-management-panel.md`](spec-05-style-management-panel.md) §10 / D1 (the sanctioned conditional-panel context pattern); unblocks [`spec-03-responsive-audit.md`](spec-03-responsive-audit.md) R-13g

---

## Context

Loki's editor shows several panels behind a condition — the style editor, the
metadata (Dublin Core) editor, the language picker, the Publish/PDF panel. Today
each is a **plain function called inside an `if`** with an early return:

```rust
// editor_inner.rs — the anti-pattern
if editing_metadata.read().is_some() {
    {metadata_panel(/* … positional args … */)}   // plain fn
}
// metadata_panel:
let draft = match editing_metadata.read().clone() {
    Some(d) => d,
    None => return rsx! {},   // early return — no component identity
};
```

That body has **no component identity**, so Dioxus gives it no hook scope: it
cannot call a hook, including the resilient
[`use_breakpoint()`](../../appthere-ui/src/responsive/mod.rs). The only way to
make such a panel responsive was to compute `compact` in the parent and **thread
a flag down** — which grew `editor_inner.rs` against its file-ceiling baseline.
This is exactly why Spec 03 **deferred R-13g** (the metadata panel's label
stacking at narrow widths): the panel could not read the breakpoint, and the
flag-threading workaround breached the ceiling.

The audit (`spec-05-style-audit.md`, SM-5/SM-6) confirmed both the problem and
that the fix already works in-tree: `FontWarning` is a `#[component]` that calls
`use_breakpoint()` successfully, and `use_breakpoint()` is resilient (returns
`Breakpoint::Expanded` when no responsive context is provided, never panics).

The style-management panel (Spec 05) is *entirely* conditionally-mounted family
panels, so it owns and defines the pattern once, here, instead of per panel.

---

## Decision

**A conditionally-shown panel is a `#[component]`, mounted at the boundary — not
a plain function called inside `if`.**

```rust
// Mount at the boundary; the component handles its own hook scope.
{open().then(|| rsx! {
    AtPanelHost { title: fl!("styles-title"), close_aria_label: fl!("close"),
                  on_close: move |_| open.set(false),
        StyleInspector { /* … */ }
    }
})}
```

Consequences of the pattern:

1. **The component owns its hook scope.** Dioxus manages it correctly across
   mount/unmount, so the panel calls `use_breakpoint()` (and any other hook)
   directly and adapts to the size class **with no `compact` flag threaded** from
   the parent.
2. **The parent does not grow.** Mounting is a single boundary expression
   (`{cond.then(|| rsx!{ … })}`); no per-panel responsive plumbing lands in
   `editor_inner.rs`, so its ceiling pressure disappears.
3. **Panels are born responsive.** Every Spec 05 family panel and sub-panel is a
   component from the start, each able to read the breakpoint.

### `AtPanelHost` — the reusable boundary

`appthere_ui::AtPanelHost` (`components/panel_host.rs`) is the shared host that
realizes the pattern: a `#[component]` that reads `use_breakpoint()` itself and
chooses posture via the pure, unit-tested `PanelPosture::for_breakpoint` — a
**full-width touch-first sheet at Compact**, a **bounded side panel at
Medium/Expanded**. It renders a titled, closable container (close control ≥
`TOUCH_MIN` = 44 px). Per the Blitz constraints (CLAUDE.md): elevation is token
border/background (**no `box-shadow`**) and it lives in flow (**no
`position: fixed`**; use `position: absolute` in a positioned ancestor when a
caller needs to dock it, per the spell-panel precedent). Spec 05 panels mount
their inspector inside it.

The responsive **decision** is a pure function (`PanelPosture::for_breakpoint`),
matching the Spec 03 D1 discipline that responsive behaviour be testable without
a real window; the component merely applies it.

---

## Consequences

- **This is a standing project convention.** New conditionally-mounted surfaces
  are components (ideally hosted by `AtPanelHost`), never plain fns in `if` with
  an early `return rsx!{}`. See the CLAUDE.md “UI conventions” note.
- **R-13g is unblocked, not yet closed.** Converting the metadata panel to a
  component lets it read the breakpoint and stack its label at Compact; making
  that change and ticking R-13g remains **Spec 03's** milestone. Spec 05 only
  establishes the pattern (and `AtPanelHost`) that R-13g needs.
- **Existing plain-fn panels are migrated opportunistically.** They are not
  broken today (they render full chrome via the `Expanded` fallback), so each is
  converted when its area is next touched — Spec 05's panels adopt the pattern
  from the start; the editor's legacy panels follow as they are revisited.
- No change to `use_breakpoint`'s resilience contract; this ADR is about *where*
  the hook may be called (a component), not the hook itself.
