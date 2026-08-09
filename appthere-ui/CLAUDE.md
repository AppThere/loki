# appthere-ui — Design System Conventions

> Cross-cutting Blitz/Dioxus-Native rules that apply to `loki-text` and every
> other UI surface — the **Confirmed/Unconfirmed CSS property** lists and the
> **ADR-0013 "conditionally-mounted panels are components"** rule — live in the
> root [`CLAUDE.md`](../CLAUDE.md), not here, so they stay loaded even when you
> are not working under `appthere-ui/`.

## Crate purpose

`appthere-ui` (crate name: `appthere_ui`) is the shared UI component library
for all AppThere suite applications: Loki Text, Loki Calc, Loki Slides (future),
Iris Photo, and Iris Draw. It provides design tokens, a theme context, and shell
components (title bar, tab bar, home tab, status bar; ribbon components are
added in subsequent passes).

## Suite structure

Each AppThere application is an independent binary. They share `appthere_ui`
for shell chrome and design tokens, but have entirely separate ribbon content,
canvas surfaces, and document models. Cross-application file type detection
is documented in the Loki Text UI specification (v0.4).

## Adding new components

1. Create a new file (or subdirectory) in `appthere-ui/src/components/`.
   File must stay under 300 lines. Split into a subdirectory proactively.
2. Define props as a `#[derive(Props, Clone, PartialEq)]` struct.
3. Re-export from `appthere-ui/src/components/mod.rs` and from `lib.rs`.
4. Use only token constants from `appthere_ui::tokens::*` — no magic numbers.
5. All interactive elements: 44×44 px minimum, documented in a doc comment.
6. Mark Dioxus Native CSS limitations with `// COMPAT(dioxus-native): ...`

## Token usage

- Colors: `appthere_ui::tokens::colors::*` — `&'static str` CSS values
- Typography: `appthere_ui::tokens::typography::*` — `&'static str` CSS values
  for font family and weight; `f32` for font sizes
- Spacing: `appthere_ui::tokens::spacing::*` — `f32` logical pixel values;
  convert to strings inline: `format!("{}px", SPACE_4)`
- Layout: `appthere_ui::tokens::layout::*` — `f32` heights and widths

## Theme context

Inject at the app root component:

```rust
provide_context(AtThemeContext::default()); // defaults to ThemeVariant::Dark
```

Read in any descendant component:

```rust
let theme = use_theme();
```

Both variants are implemented (4c.4): `ThemePalette::dark()` / `light()` in
`tokens/palette.rs`; the `COLOR_*` constants remain the dark values, so
unmigrated components render dark under either variant. Components migrate by
reading `use_theme().palette()` (Signal-backed — re-colors live on
`AtThemeContext::toggle`, exposed as the tab-bar theme-toggle button). Shell
chrome (title/tab/status bars, dialogs) is migrated; deep editor surfaces
migrate opportunistically.

## What does NOT belong in `appthere_ui`

- Document rendering (Vello, Parley, Loro)
- Format-specific code (OOXML, ODF, EPUB)
- Application-specific business logic or routing
- Ribbon tab content (each application provides its own — `AtRibbon` with
  a children/slot API is implemented in a future pass)
