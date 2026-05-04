# Workspace Dependency Patches

This file documents every `[patch]` entry in the root `Cargo.toml`.
Update this file whenever a patch is added, modified, or removed.

## Why we patch

Loki targets Dioxus Native 0.7 and Blitz, both of which are pre-1.0 crates
with evolving APIs. Patches are the correct Rust mechanism for working around
upstream gaps while those gaps are being resolved. Each patch below is
temporary and has a documented removal condition.

---

## Active patches

### fontique — 0.8.0

**Source:** `patches/fontique/` (local), vendored from upstream commit
`8dbecc0545a0c97eb605937b928bc186d2d1295c` in
[linebender/parley](https://github.com/linebender/parley) (`fontique/` path
in that monorepo).

**Fixes:** Two related issues with the crates.io publication of fontique 0.8.0:

1. **Missing package alias.** The crates.io publication lost the
   `fontconfig_sys = { package = "yeslogic-fontconfig-sys", ... }` alias
   during Cargo normalization. The source in
   `src/backend/fontconfig.rs` uses `use fontconfig_sys::…` and requires
   this alias to compile.

2. **Workspace feature-unification conflict.** `blitz-dom` depends on
   fontique 0.6.0 and activates `yeslogic-fontconfig-sys/dlopen` for the
   entire build graph. Without the patch, fontique 0.8.0 is built without
   dlopen and fails on static-C imports because the two versions of the
   yeslogic-fontconfig-sys feature cannot agree. The patch enables
   `fontconfig-dlopen` on 0.8.0 so both versions use the same linkage mode.

**Root cause:** Bug in the fontique 0.8.0 crates.io publish pipeline (package
alias dropped during Cargo manifest normalisation). Compounded by Cargo
feature-unification behaviour when two semver-incompatible versions of
fontique coexist in the dependency graph.

**Upstream status:** No upstream issue filed as of 2026-05-03. Upstream
repository is [linebender/parley](https://github.com/linebender/parley).

**Removal condition:** Remove when a post-0.8.0 fontique release on crates.io
restores the `fontconfig_sys` package alias and the dlopen/static linkage
conflict is resolved (either fontique 0.8 is no longer paired with blitz-dom's
fontique 0.6, or upstream aligns feature flags).

**Added:** 2026-04-13 (introduced in the loki-text scaffold commit).

---

### dioxus-native-dom — 0.7.4

**Source:** `patches/dioxus-native-dom/` (local), vendored from upstream
commit `1eb00b5e0080ab4bd6a11ddd0a01c97f28493e04` in
[DioxusLabs/dioxus](https://github.com/DioxusLabs/dioxus)
(`packages/native-dom/` path). The vendor copy carries local modifications
(`dirty: true` in `.cargo_vcs_info.json`).

**Fixes:** The upstream dioxus-native-dom 0.7.4 panics at runtime for any
event type whose `HtmlEventConverter` implementation is a placeholder
`unimplemented!()`. The affected methods include:

- `convert_composition_data` — called for IME (CJK/RTL) input
- `convert_touch_data` — called for all touch events
- `convert_pointer_data`, `convert_scroll_data`, `convert_wheel_data`
- `convert_cancel_data`, `convert_clipboard_data`, `convert_drag_data`,
  `convert_image_data`, `convert_media_data`, `convert_mounted_data`,
  `convert_animation_data`, `convert_selection_data`, `convert_toggle_data`,
  `convert_transition_data`, `convert_resize_data`, `convert_visible_data`

Vendoring the crate locally means Loki can build against a known snapshot and
apply targeted fixes without being blocked by an upstream release. See
`docs/editing/input-event-audit.md` — the **Blockers** section — for a
detailed event-by-event analysis of what works and what panics.

**Root cause:** dioxus-native-dom 0.7.4 is a pre-1.0 crate; many
`HtmlEventConverter` methods are unimplemented stubs that panic if called.
Upstream is aware (the `todo:` message in each `unimplemented!()` call names
the missing blitz support), but the fixes depend on blitz-dom adding the
corresponding event infrastructure.

**Upstream status:** No standalone issue filed as of 2026-05-03. The
unimplemented converters are tracked locally in
`docs/editing/input-event-audit.md`. Upstream repository is
[DioxusLabs/dioxus](https://github.com/DioxusLabs/dioxus).

**Removal condition:** Remove when dioxus-native-dom upstream implements the
event converters Loki requires — at minimum `convert_composition_data` (IME)
and `convert_touch_data` (mobile) — and publishes a 0.7.x release that does
not panic for those paths. Before removing, verify with the event availability
table in `docs/editing/input-event-audit.md` that all "Required for editing"
events are available without panicking.

**Added:** 2026-05-02 (introduced in the cursor positioning commit).

---

## Removing a patch

Before removing a patch:

1. Confirm the upstream release that fixes the issue is in `Cargo.lock`.
2. Remove the `[patch]` entry from `Cargo.toml`.
3. Run `cargo check --workspace` and `cargo test --workspace`.
4. Remove the patch source directory (`patches/<crate>/`).
5. Update or remove the corresponding entry in this file.
