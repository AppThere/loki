<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
|------|----------|
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.

---

## Coding conventions

These conventions apply to all crates in the workspace.

- **File ceiling:** No `.rs` file may exceed 300 lines. Split files proactively
  before hitting the ceiling — do not write a large file and split reactively.
- **Error handling:** Use typed error enums with `thiserror`. Do not use `anyhow`
  in library crates. Do not use `.unwrap()` or `.expect()` in library code
  outside of `#[cfg(test)]` blocks.
- **Unsafe:** `#![forbid(unsafe_code)]` must be present in `lib.rs` for all
  `appthere-ui` and future library crates.
- **License header:** Every `.rs` file must begin with:
  `// SPDX-License-Identifier: Apache-2.0`
- **Annotations:**
  - `// COMPAT(dioxus-native): <explanation>` — marks any workaround for a
    Dioxus Native / Blitz CSS or API limitation, including unconfirmed CSS
    properties that need runtime verification.
  - `// TODO(<topic>): <description>` — marks deliberately deferred work.
- **No hardcoded user-visible strings:** All display strings are props or named
  constants. The `loki_i18n` crate (Fluent-based) will provide formatted
  strings to `appthere_ui` components; the components are i18n-agnostic.
- **Touch targets:** Every interactive component must document its minimum
  44×44 logical pixel touch target (WCAG 2.5.8) in a doc comment.
- **Checkpoints:** Run `cargo check --workspace` after each logical unit of
  work. Do not accumulate failures across steps.
- **Documentation Sync:** Any change to layout, rendering, or import/export properties must update the living status registry in [fidelity-status.md](file:///Users/kevin/project/loki/docs/fidelity-status.md).
- **Final pass:** `cargo fmt --all` and `cargo clippy --workspace -- -D warnings`
  must both pass before any PR or commit is considered complete.

### Clippy compliance

The entire workspace must pass `cargo clippy --workspace -- -D warnings`.

For pre-existing code in `loki-layout`, `loki-odf`, and `loki-ooxml` that
required structural changes beyond the scope of the cleanup pass, targeted
`#[allow(clippy::rule_name)]` attributes were added at the narrowest applicable
scope (function or struct level, never crate level) with a comment explaining why.

When adding `#[allow]` to any file:
- Scope it as narrowly as possible (prefer function-level over file-level)
- Add a comment: `// Pre-existing pattern — structural refactor deferred`
  or explain the specific reason the lint does not apply
- Never suppress `clippy::correctness` lints (these indicate bugs)
- Prefer simple fixes over `#[allow]` wherever the change is mechanical
  and low-risk

---

## Known tech debt (300-line ceiling violations — requires future split pass)

These files existed before the ceiling convention was established and have not
yet been split. Do not add new code to them without splitting first.

| File | Current lines | Priority |
|---|---|---|
| `loki-text/src/components/document_source.rs` | 1117 | High |
| `loki-doc-model/src/loro_bridge/inlines.rs` | ~280 | Low |

(`read.rs` was split into `read.rs` + `props_read.rs`; both are now under 300 lines.)

## Known tech debt — Loro bridge round-trip gaps

Several `ParaProps`/`CharProps` fields are read from OOXML/ODF during import
but are **not perfectly round-tripped through the Loro CRDT**.

| Field(s) | Status | Priority |
|---|---|---|
| `tab_stops` | Written as unreadable Debug string; not read back. | Medium |
| `background_color` (paragraph) | Written as Debug string; not decoded on read. | Low |

---

## appthere-ui — Design System Conventions

### Crate purpose

`appthere-ui` (crate name: `appthere_ui`) is the shared UI component library
for all AppThere suite applications: Loki Text, Loki Calc, Loki Slides (future),
Iris Photo, and Iris Draw. It provides design tokens, a theme context, and shell
components (title bar, tab bar, home tab, status bar; ribbon components are
added in subsequent passes).

### Suite structure

Each AppThere application is an independent binary. They share `appthere_ui`
for shell chrome and design tokens, but have entirely separate ribbon content,
canvas surfaces, and document models. Cross-application file type detection
is documented in the Loki Text UI specification (v0.4).

### Adding new components

1. Create a new file (or subdirectory) in `appthere-ui/src/components/`.
   File must stay under 300 lines. Split into a subdirectory proactively.
2. Define props as a `#[derive(Props, Clone, PartialEq)]` struct.
3. Re-export from `appthere-ui/src/components/mod.rs` and from `lib.rs`.
4. Use only token constants from `appthere_ui::tokens::*` — no magic numbers.
5. All interactive elements: 44×44 px minimum, documented in a doc comment.
6. Mark Dioxus Native CSS limitations with `// COMPAT(dioxus-native): ...`

### Confirmed CSS properties (Dioxus Native 0.7.x / Blitz)

These work in production code:

- `display: flex`, `flex-direction`, `flex-shrink`, `flex: 1`, `align-items`, `gap`
- `overflow-x: auto`, `overflow-y: auto`
- `border-radius: Npx`
- `position: relative`, `z-index: N`
- `height: calc(100vh - Npx)`, `width: 100vw`, `height: 100vh`
- `border-bottom/top: Npx solid COLOR`
- `box-sizing: border-box`

### Unconfirmed CSS properties — verify at runtime, mark with COMPAT comment

- `opacity: 0.N` (needed for disabled-state rendering)
- `white-space: nowrap` (needed for tab label truncation)
- `text-overflow: ellipsis` (needed for tab label truncation)
- `overflow-x: scroll` (explicit scroll vs. auto)
- `scrollbar-width: none` (needed for invisible scroll containers)
- `position: absolute` — **confirmed unsupported** in current Blitz.
  Tooltip components are deferred until this is resolved.
- `position: fixed` — **confirmed unsupported** (documented in toolbar.rs).

### Token usage

- Colors: `appthere_ui::tokens::colors::*` — `&'static str` CSS values
- Typography: `appthere_ui::tokens::typography::*` — `&'static str` CSS values
  for font family and weight; `f32` for font sizes
- Spacing: `appthere_ui::tokens::spacing::*` — `f32` logical pixel values;
  convert to strings inline: `format!("{}px", SPACE_4)`
- Layout: `appthere_ui::tokens::layout::*` — `f32` heights and widths

### Theme context

Inject at the app root component:

```rust
provide_context(AtThemeContext::default()); // defaults to ThemeVariant::Dark
```

Read in any descendant component:

```rust
let theme = use_theme();
```

Only `ThemeVariant::Dark` is implemented. Light theme tokens are deferred.

### What does NOT belong in `appthere_ui`

- Document rendering (Vello, Parley, Loro)
- Format-specific code (OOXML, ODF, EPUB)
- Application-specific business logic or routing
- Ribbon tab content (each application provides its own — `AtRibbon` with
  a children/slot API is implemented in a future pass)

---

## Internationalisation (loki-i18n)

### Never hardcode user-visible strings

All user-visible strings in `loki-text` and future Loki suite apps must use
`loki_i18n::fl!()`. No string literals in RSX or prop assignments.

### Adding new strings

1. Add the key and en-US value to the appropriate `.ftl` file in
   `loki-i18n/i18n/en-US/`. Domain mapping:
   - `shell.ftl` — persistent shell chrome (tab bar, window)
   - `home.ftl` — Home screen
   - `editor.ftl` — document editor chrome (status bar, zoom)
   - `ribbon.ftl` — ribbon tabs and controls
   - `errors.ftl` — error messages shown to the user
   - `document.ftl` — document-level labels (save, export, etc.)
2. Use the string in code via `fl!("your-key")`.
3. For strings with arguments: `fl!("key", arg = value)`.
   Integer arguments must be `i64`, float arguments `f64`.

### Key naming convention

`{domain}-{component}-{description}` in kebab-case.
Examples: `shell-home-tab`, `editor-page-label`, `home-no-recent`.

### Adding a new locale

1. Create `loki-i18n/i18n/{locale}/` (e.g. `fr-FR/`).
2. Copy all `.ftl` files from `en-US/` and translate the values.
3. Keys must remain identical to `en-US` — only values are translated.
4. Missing keys fall back to `en-US` automatically at runtime.

### Props that accept translated strings

`appthere_ui` component props that display text use `String` (not
`&'static str`) so translated strings can be passed. Pass `fl!("key")`
directly — no intermediate `let` binding needed.

### Macro internals

`fl!()` is defined in `loki-i18n/src/lib.rs`. It expands to a call on the
global `OnceLock<LokiBundle>` (initialised by `loki_i18n::init()` in
`main.rs`). Callers do not need `fluent` as a direct dependency — it is
re-exported as `loki_i18n::fluent` for use inside the macro expansion.
