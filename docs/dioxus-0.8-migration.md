# Dioxus 0.8 migration — survey and plan

**Status:** exploratory. Nothing has been changed in the tree; this document is
the survey and the plan.

**Surveyed against:** `dioxus 0.8.0-alpha.1` (latest 0.8 alpha on crates.io as of
2026-08-16; `0.8.0-alpha.0` preceded it, and `0.7.10` was published after
`0.8.0-alpha.0`). Current pin: `=0.7.9`.

**Method.** Upstream `.crate` tarballs for both stacks were downloaded and
diffed directly (0.7.9/0.2.x → 0.8.0-alpha.1/0.3.0-beta.1), and each of the
seven local patches was diffed against its own upstream baseline to separate
"what Loki changed" from "what upstream changed underneath it". Everything
marked **Observed** below is read off those diffs. Everything else is marked
**Not established**, with the check that would settle it.

---

## 1. Verdict up front

This is not a version bump. Migrating to 0.8 means:

1. **Rewriting the entire GPU page-tile integration.** `CustomPaintSource` /
   `CustomPaintCtx` / `use_wgpu` — the API `loki-renderer` is built on — **do
   not exist in the 0.8 stack**. They are replaced by a differently-shaped
   `blitz_dom::Widget` custom-element API.
2. **Re-vendoring five patches against a near-rewritten Blitz.** `blitz-dom`
   changed 8 825 of ~10 511 source lines between 0.2.4 and 0.3.0-beta.1. Loki's
   574-line patch against it is a reimplementation, not a rebase.
3. **A simultaneous graphics-stack bump**: wgpu 26 → 29, vello 0.6 → 0.9,
   peniko 0.5 → 0.6, kurbo 0.12 → 0.13, winit 0.30 → 0.31-beta.2, taffy 0.9 →
   0.12, parley 0.6 → 0.10, markup5ever 0.35 → 0.39.

Against that, 0.8 **retires a large amount of Loki-specific patch surface**:
scroll, wheel, touch, pointer and `mounted` events all land upstream, along with
both currently-documented stack deviations. The migration is worth doing — the
question is sequencing, and the honest read is that it should not start until
0.8 is at least at release-candidate.

**Recommendation:** do the preparatory work now (§6 Phase 0), which is valuable
whether or not 0.8 ships on schedule, and hold the stack bump itself until
`dioxus-native` 0.8 reaches rc. The blocking unknown (§5, R-1) is whether the
165 MiB single-Renderer optimisation survives; that is worth resolving with
upstream *before* committing to the port, not after.

---

## 2. The dependency landscape

**Observed** — from the crates.io index and the crate manifests:

| Crate | today (0.7.9 stack) | 0.8.0-alpha.1 stack |
|---|---|---|
| `dioxus` | `=0.7.9` | `=0.8.0-alpha.1` |
| `dioxus-native`, `dioxus-native-dom` | 0.7.9 | 0.8.0-alpha.1 |
| `blitz-{dom,shell,net,paint,traits,html}` | 0.2.x | `=0.3.0-beta.1` (exact-pinned by dioxus-native) |
| `anyrender` | 0.6.x | 0.11 |
| `anyrender_vello` | 0.6.2 | 0.12 (now **optional**) |
| `winit` | 0.30.11 | `=0.31.0-beta.2` (+ new `winit-android` crate) |
| `wgpu` | 26 | 29 |
| `vello` | 0.6 | 0.9 |
| `peniko` / `kurbo` | 0.5 / 0.12 | 0.6 / 0.13.1 |
| `parley` (Blitz side) | 0.6 | **0.10** |
| `taffy` / `stylo_taffy` | 0.9 / 0.2 | 0.12.1 / `=0.3.0-beta.1` |
| `markup5ever` | 0.35 | 0.39 |
| `android-activity` | 0.6 | **0.6 — unchanged** |

Two consequences worth calling out separately:

- **`android-activity` does not move.** winit 0.31 factors Android into a
  `winit-android` crate, which still requires `android-activity ^0.6.0` and
  `ndk ^0.9.0`. The `android_main!` FFI entry point and the unsafe-policy
  allowlist are unaffected.
- **The default renderer changed.** `dioxus-native` 0.8's default feature set is
  `vello-hybrid`, not `vello`, and `autofocus` is no longer defaulted. Loki
  needs full `vello` for the texture path. Since the `dioxus` meta-crate's
  `native` feature only does `dep:dioxus-native` with defaults, **Loki should
  take a direct `dioxus-native` dependency** with `default-features = false` and
  an explicit feature list. That alone retires one of the two reasons the
  `dioxus-native` patch exists (§4).

---

## 3. What breaks, by area

### 3.1 GPU page tiles — the big one

**Observed.** `dioxus-native` 0.7.9 re-exports
`anyrender_vello::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle}`
and `dioxus_renderer::use_wgpu`. In 0.8.0-alpha.1 **none of these exist**;
`anyrender_vello` 0.12 has no `custom_paint_source.rs` at all.

The replacement is a custom-widget API:

- `blitz_dom::Widget` — `connected` / `disconnected` / `attribute_changed` /
  `can_create_surfaces(&mut dyn RenderContext)` / `destroy_surfaces` /
  `handle_event(&UiEvent)` / `paint(&mut dyn RenderContext, &ComputedStyles, w, h, scale) -> Scene`.
- Attached from RSX via `dioxus_native_dom::CustomWidgetAttr::new(widget)` set as
  the `data` attribute of an `<object>` element (`mutation_writer.rs:223-237`).
- A widget renders to wgpu by taking the `DeviceHandle` out of
  `RenderContext::renderer_specific_context()` (a `Box<dyn Any>` downcast),
  creating its **own** `wgpu::Texture`, registering it with
  `RenderContext::try_register_custom_resource` → `ResourceId`, and drawing that
  id as an image into the returned `Scene`.

Files affected (`loki-renderer`, ~1 000+ lines): `page_paint_source.rs`,
`page_paint_lifecycle.rs`, `page_paint_render.rs`, `page_tile.rs`,
`renderer_state.rs`, `doc_page_source.rs`, `doc_page_source_scale.rs`,
`dpr_probe.rs`, `gpu_probe.rs`, plus `page_source_impl.rs` and the
`appthere-canvas` residency accounting that hangs off the texture lifecycle.

Mapping notes:

- `resume(&DeviceHandle)` → `can_create_surfaces(&mut dyn RenderContext)` +
  downcast. Loki's own upstreamed-in-patch `release(ctx)` hook maps cleanly onto
  `destroy_surfaces` / `disconnected` + `RenderContext::unregister_resource`, so
  the texture-leak fix that hook exists for is now expressible without a patch.
- The `scale` argument survives (`paint(..., scale: f64)`), so `dpr_probe`'s
  one-frame-lagged DPR read has an equivalent landing site.
- `paint` now also receives `&ComputedStyles`, which is strictly more than
  today's `render` gets.

### 3.2 Events — mostly *fixed* upstream

**Observed.** `blitz-traits` 0.3 `DomEventData` grows from 9 variants to ~30,
adding `Scroll`, `Wheel`, `TouchStart/Move/End/Cancel`, the full `Pointer*` set,
`MouseEnter/Leave/Over/Out`, `ContextMenu`, `DoubleClick`, `FocusIn/Out`.
`dioxus-native-dom` 0.8 implements `convert_scroll_data`, `convert_wheel_data`,
`convert_touch_data`, `convert_pointer_data` and `convert_mounted_data`, and
ships a `NodeHandle: RenderedElementBacking` with `scroll`, `scroll_to`,
`get_scroll_offset`, `get_scroll_size`, `get_client_rect` and `set_focus`.
Dispatch walks the ancestor chain to the nearest Dioxus-mapped node
(`dioxus_document.rs:334-337`) — precisely the bubbling behaviour Loki's wheel
patch added.

That is essentially the whole of `patches/dioxus-native-dom` (504 changed lines)
plus the touch-forwarding half of `patches/blitz-shell`, landing upstream.

**Both documented stack deviations in `docs/patches.md` also close:**
`mouseenter`/`mouseleave` now exist as event variants, and `PointerCoords`
carries `page_x/page_y` **and** `client_x/client_y` separately — so the
"`clientX`/`clientY` are page coordinates and `pageX`/`pageY` are missing"
deviation is resolved at the source.

**The one residual gap.** Loki's `NativeWheelData` carries a
**scrollport-relative** position, consumed by
`loki-text/src/routes/editor/editor_wheel_zoom.rs:153` (`data.scrollport`) to
anchor Ctrl+wheel zoom. Upstream's `BlitzWheelEvent` carries `coords`
(page/client) and `element` (target-relative) only, and the scrollport frame
cannot be derived from those by the consumer. This needs either a small
retained patch or re-derivation from `get_client_rect` on the scroll container.

**Still `unimplemented!()` in 0.8**: composition (IME), clipboard, selection,
drag, animation, media, image, transition, resize, visible, cancel,
beforeinput. Composition was never implemented in Loki's patch either, so this
is not a regression — Android IME continues to depend on
`patches/blitz-shell/src/ime_android.rs`.

### 3.3 Launch / config API

**Observed.** `Config::with_fonts(Vec<Vec<u8>>)` — used by
`loki-text/src/main.rs:65` — is a **Loki patch method**, not upstream. 0.8 ships
`Config::with_font_ctx(FontContext)` plus `blitz_dom::build_single_font_ctx` for
the one-font case. Loki registers many blobs (`loki_fonts::ui_font_blobs()`), so
it must build a multi-font `FontContext` and pass that instead.

`winit` 0.31 renames apply to the launch path as well
(`WindowAttributes::with_inner_size` → `with_surface_size`; `Window` is now
`Arc<dyn Window>`). Affects `loki-text/src/main.rs`, the `loki-spreadsheet` and
`loki-presentation` entry points, and `appthere-ui/examples/{linebreak,nested_scroll}_probe.rs`.

New and useful: `use_window_event` and `use_back_button` hooks — the latter is
the Android hardware back button, which Loki currently has no first-class path
for.

### 3.4 Graphics crates

`loki-vello` (`vello 0.6`, `peniko 0.5`, `kurbo 0.12`, `wgpu 26`),
`appthere-canvas` (`wgpu 26`, `peniko 0.5`), `loki-renderer` (`wgpu 26`,
`vello 0.6`, `anyrender_vello 0.6.2`) and `loki-graphics` must all move in
lockstep to wgpu 29 / vello 0.9 / peniko 0.6 / kurbo 0.13, because the tile
textures are handed to Blitz's renderer and both sides must agree on the wgpu
version.

**Not established:** the size of the vello 0.6 → 0.9 API delta for the surface
Loki actually uses (`Renderer::new`, `render_to_texture`, `RenderParams`,
`AaConfig`, `Scene`). *What would settle it:* a scratch crate compiling
`loki-vello` against vello 0.9 in isolation — it has no Blitz dependency, so
this can be done today, before any other migration work.

### 3.5 Layout and CSS

taffy 0.9 → 0.12.1, `stylo_taffy` 0.2 → 0.3, a Stylo bump, and markup5ever
0.35 → 0.39 all land at once. `dioxus-native` 0.8 additionally gains `floats`
and `incremental` features (the latter now on by default).

**Not established:** whether the "Confirmed CSS properties" list in `CLAUDE.md`
still holds — in particular `position: absolute` (which the floating spelling
context menu depends on, and which was only confirmed working on the *current*
Stylo + stylo_taffy 0.2 + Taffy 0.9 stack) and whether `position: fixed` still
collapses to `absolute`. *What would settle it:* re-run the existing runtime
probes (`appthere-ui/examples/*_probe.rs`) against the 0.8 stack and re-record
the results in `CLAUDE.md` and `docs/fidelity-status.md`.

### 3.6 Dioxus core / macros

- `#[component]`-derived props structs are now `#[non_exhaustive]`. Loki's
  hand-written `#[derive(Props)]` structs are unaffected; any *construction* of
  a macro-derived props struct by literal would break. Grep shows none, but this
  is worth a compile-driven check rather than a grep.
- Stores/signals: `ReadStore` now requires `Lens: Readable`; store derives
  enforce method/field visibility. Loki uses `Signal`/`WritableExt` but no
  `dioxus-stores` derives, so exposure looks low.
- Dioxus moved to edition 2024. Loki is already predominantly edition 2024
  (~46 crates) with a handful still on 2021; the pinned 1.97.1 toolchain
  supports it either way.

---

## 4. Patch disposition

Seven `[patch]` entries touch this stack. `loki-file-access` and
`appthere-color` are unrelated and unaffected.

| Patch | Loki's delta | Upstream churn underneath | Disposition |
|---|---|---|---|
| **`parley` 0.6.0** | 2 lines | — | **Drop.** blitz-dom 0.3 uses parley **0.10**, which carries the letter/word-spacing fix. This also collapses the dual-shaper deviation: Blitz and `loki-layout` would run the *same* parley version for the first time. |
| **`dioxus-native-dom`** | 504 lines | 690 of 924 lines | **Mostly drop.** Scroll/wheel/touch/pointer/mounted all upstream. Retain only: `NativeWheelData.scrollport` (§3.2), and re-verify the onmounted pending-drain and the focus-preservation-across-render behaviour against 0.8's own implementations. |
| **`dioxus-native`** | 281 lines | 442 of 674 lines | **Mostly drop.** `with_fonts` → upstream `with_font_ctx`; `autofocus` → feature selection on a direct dependency. Retain only if the Android `request_redraw`-after-`CreateHeadElement` fix is still needed — 0.8's incremental-rendering redraw path may already cover it. |
| **`blitz-shell`** | 836 lines (incl. whole files `ime_android.rs`, `tooltip.rs`) | 1 222 of 1 514 lines (~81 %) | **Re-implement.** Touch forwarding is obsolete (winit 0.31 pointer events, `convert_events.rs:69-107`). `ime_android.rs` and `tooltip.rs` are self-contained subsystems that must be re-vendored against the rewritten `window.rs`. |
| **`blitz-dom`** | 574 lines across 10 files | **8 825 of 10 511 lines (~84 %)** | **Re-implement from scratch.** `scroll_node_by` now exists upstream with a dispatch callback. Still absent upstream: `scrollport_origin`, `is_tab_focussable` (tab order ≠ focusability), `scroll_axes`, the accesskit stale-focus guard, and the focus-on-click fix — `handle_click` still calls `clear_focus()` on no match (`events/pointer.rs:742`). |
| **`blitz-net`** | manifest-only (0 src lines) | 346 lines | **Re-decide.** 0.3 still defaults to `native-tls`, but now adds a `cfg(target_os = "android")` override using `native-tls-vendored` (vendors OpenSSL rather than linking `libssl.so`). Loki's rustls override is still expressible as a manifest-only patch; whether it is still *needed* depends on whether the vendored build is acceptable for APK size. |
| **`anyrender_vello`** | 229 lines | 408 of 560 lines | **Split.** The Android Mali `num_init_threads = 1` workaround is **still needed** — 0.12 still applies it for macOS only (`lib.rs:18-21`). `release(ctx)` is superseded by the `Widget` lifecycle. `renderer_mut()` has **no 0.8 home** — see R-1. |

---

## 5. Risks and open questions

**R-1 — the 165 MiB Renderer-sharing optimisation has no 0.8 equivalent.**
*Observed:* commit `9de9a8d` moved page tiles onto Blitz's own `vello::Renderer`
via a Loki-added `CustomPaintCtx::renderer_mut()`, worth ~165 MiB (a second
`vello::Renderer` allocates fixed-size scratch buffers regardless of scene). In
0.8, `VelloScenePainter.renderer` is `pub(crate)`, and
`RenderContext::renderer_specific_context()` returns **only** a cloned
`DeviceHandle` — the `Renderer` is not reachable from a `Widget`.
*Not established:* whether upstream would accept exposing it, and whether the
`Widget` texture path re-introduces the second-Renderer cost by construction.
*What would settle it:* raise it with Blitz/Dioxus upstream **before** starting
the port — this is the single question most likely to change the shape of the
`loki-renderer` rewrite. Second-best: prototype a `Widget` that registers a
texture and measure resident GPU memory against today's baseline.

**R-2 — 0.3.0-beta.1 is exact-pinned by `dioxus-native`.** Every Blitz crate is
`=0.3.0-beta.1`. Loki's own direct `blitz-dom`/`blitz-html`/`blitz-traits`/
`blitz-shell` dependencies (in `loki-text`, `loki-presentation`,
`loki-spreadsheet`) must be pinned to the identical version or the graph
duplicates and the patches silently stop applying — the same failure mode the
existing `=0.7.9` pin exists to prevent.

**R-3 — alpha churn.** Between `0.8.0-alpha.0` and `0.8.0-alpha.1` the Blitz pin
moved `0.3.0-alpha.4` → `0.3.0-beta.1` and every anyrender crate bumped a major.
Re-vendoring 1 400+ lines of patch against a moving target is the dominant cost
of migrating early rather than at rc.

**R-4 — CSS/layout regressions are invisible until run.** §3.5. The confirmed-CSS
list and `docs/fidelity-status.md` are the affected records.

**R-5 — the ACID fidelity harness is the only real acceptance gate.** Whether
`loki-acid`'s page-count/glyph-coverage canaries and SSIM comparisons still pass
after a parley 0.6 → 0.10 shaper change on the Blitz side is unknown and cannot
be predicted from source reading.

---

## 6. Plan

### Phase 0 — preparation (can start now, no 0.8 dependency)

Everything here is useful even if 0.8 slips, and each item shrinks the eventual
port.

- **0.1** Take a **direct `dioxus-native` dependency** with an explicit feature
  list instead of relying on `dioxus/native` defaults. Retires the `autofocus`
  half of the `dioxus-native` patch today.
- **0.2** Bump `loki-vello` to vello 0.9 / peniko 0.6 / kurbo 0.13 / wgpu 29 in
  isolation. It has no Blitz dependency, so this is independently landable and
  measures §3.4's unknown.
- **0.3** Raise R-1 with upstream Blitz/Dioxus (expose the `Renderer` to custom
  widgets, or document the intended zero-copy path).
- **0.4** Write down the current runtime-probe baselines (CSS probes, texture
  residency at a known document + zoom, ACID canary results) so the post-migration
  comparison has a *floor as well as a ceiling* — a migration that reports
  "improved" memory is exactly the one nobody double-checks.
- **0.5** Re-derive `editor_wheel_zoom`'s scrollport anchor from public geometry
  (`get_client_rect` on the scroll container) if possible, removing the last
  reason to keep a `dioxus-native-dom` patch.

### Phase 1 — the stack bump (gated on 0.8 rc, or on R-1 being answered)

- **1.1** Bump the pin to `=0.8.0-rc.N` across the workspace; pin every direct
  Blitz dependency to the exact version `dioxus-native` requires (R-2).
- **1.2** Delete the `parley` patch. Confirm a single parley version resolves
  workspace-wide.
- **1.3** Re-vendor `anyrender_vello` carrying **only** the Android
  `num_init_threads` workaround (plus the image-brush-transform hunk if still
  needed).
- **1.4** Re-vendor `blitz-net` as a manifest-only rustls override, or drop it if
  upstream's `native-tls-vendored` Android path is acceptable on APK size.
- **1.5** Expect the workspace not to compile until Phase 2 lands. Track it as
  one branch, not a series of half-migrated commits.

### Phase 2 — the two rewrites

- **2.1 `loki-renderer` on `Widget`.** Port `LokiPageSource` to
  `blitz_dom::Widget`; move device acquisition to `can_create_surfaces`, texture
  registration to `try_register_custom_resource`, teardown to
  `destroy_surfaces`/`disconnected`. Replace `use_wgpu(...)` + canvas-id
  plumbing in `page_tile.rs` with `object { data: CustomWidgetAttr::new(...) }`.
  Keep `TileKey`, the residency budget, and the raster-permille concession
  intact — they are renderer-agnostic and should survive unchanged.
- **2.2 `blitz-dom` / `blitz-shell` patches, re-implemented.** Against ~84 % and
  ~81 % rewritten upstreams, port each Loki behaviour individually and re-justify
  it against the new code rather than re-applying hunks: focus-on-click,
  accesskit stale-focus, tab-order, scrollport geometry, `ime_android`, `tooltip`.
  Several may turn out to be unnecessary; that is a finding, not a shortcut.

### Phase 3 — patch retirement and re-verification

- **3.1** Delete the obsolete parts of `dioxus-native-dom` and `dioxus-native`,
  and the touch-forwarding part of `blitz-shell`. Update `docs/patches.md` in the
  same change — including moving both now-fixed stack deviations into the
  "Removed" section with the version that fixed them.
- **3.2** Re-run the CSS runtime probes; update the confirmed/unconfirmed lists
  in `CLAUDE.md` and `docs/fidelity-status.md`.
- **3.3** Run the full ACID harness and compare against the Phase 0.4 baselines,
  in both directions.
- **3.4** Re-verify on Android device: Mali shader init, IME, touch scroll and
  long-press selection, back button (now expressible via `use_back_button`),
  APK size.
- **3.5** `cargo fmt --all --check` and the exact CI clippy command.

### Not in scope

Adopting 0.8's new native APIs (SwiftUI/Kotlin widgets, portals, the FFI
system, `Config::floats`). Those are follow-on work; folding them into the port
would make the regression surface unattributable.

---

## 7. What this survey did not check

Stated plainly so the gaps are not mistaken for clean results:

- No compilation was attempted against 0.8 — every claim here is read off
  upstream source diffs, not off a build.
- The vello 0.6 → 0.9 and wgpu 26 → 29 API deltas were not enumerated (§3.4).
- `dioxus-router` 0.8's delta was not examined; `loki-text`,
  `loki-spreadsheet` and `loki-presentation` all enable the `router` feature.
- The Stylo/taffy CSS behaviour delta was not assessed beyond noting the version
  jump (§3.5).
- Whether `blitz-dom` 0.3's `flush_is_focussable` already covers the
  `tabindex="0"` div case that Loki's focus-on-click patch exists for was not
  determined — it is a source read away, but the answer only matters once 2.2
  starts.
