# Internationalisation (loki-i18n)

> The always-on rule — **never hardcode user-visible strings; use
> `loki_i18n::fl!()`** — lives in the root [`CLAUDE.md`](../CLAUDE.md) because it
> applies when writing any UI code. This file holds the mechanics, loaded when
> you work under `loki-i18n/`.

## Adding new strings

1. Add the key and en-US value to the appropriate `.ftl` file in
   `loki-i18n/i18n/en-US/`. Domain mapping:
   - `shell.ftl` — persistent shell chrome (tab bar, window)
   - `home.ftl` — Home screen
   - `editor.ftl` — document editor chrome (status bar, zoom)
   - `ribbon.ftl` — ribbon tabs and controls
   - `errors.ftl` — error messages shown to the user
   - `document.ftl` — document-level labels (save, export, etc.)
   - `publish.ftl` — Publish tab: PDF/EPUB export and Dublin Core metadata
2. Use the string in code via `fl!("your-key")`.

   **Adding a whole new domain** (a new `.ftl` file) requires registering it in
   the `DOMAINS` array in `loki-i18n/src/loader.rs` — files are not
   auto-discovered.
3. For strings with arguments: `fl!("key", arg = value)`.
   Integer arguments must be `i64`, float arguments `f64`.

## Key naming convention

`{domain}-{component}-{description}` in kebab-case.
Examples: `shell-home-tab`, `editor-page-label`, `home-no-recent`.

## Adding a new locale

1. Create `loki-i18n/i18n/{locale}/` (e.g. `fr-FR/`).
2. Copy all `.ftl` files from `en-US/` and translate the values.
3. Keys must remain identical to `en-US` — only values are translated.
4. Missing keys fall back to `en-US` automatically at runtime.

## Props that accept translated strings

`appthere_ui` component props that display text use `String` (not
`&'static str`) so translated strings can be passed. Pass `fl!("key")`
directly — no intermediate `let` binding needed.

## Macro internals

`fl!()` is defined in `loki-i18n/src/lib.rs`. It expands to a call on the
global `OnceLock<LokiBundle>` (initialised by `loki_i18n::init()` in
`main.rs`). Callers do not need `fluent` as a direct dependency — it is
re-exported as `loki_i18n::fluent` for use inside the macro expansion.
