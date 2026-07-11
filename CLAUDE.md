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

## Engineering principles — fix the cause, not the symptom

**Always prefer the correct, root-cause fix over a quick patch.** A change is
not done when the symptom disappears; it is done when you understand *why* the
symptom occurred and have addressed that cause. This is a hard expectation, not
a preference.

- **Diagnose before you change.** Reproduce the issue, find the actual cause
  (read the code, add temporary diagnostics, bisect), and state it explicitly
  before editing. If you cannot explain the mechanism, you are not ready to fix
  it. (Example: the "frozen scrollbar" bug was not in the editor at all — it was
  a `[patch]` silently dropped by a dioxus version bump; the fix was to restore
  the patch, not to rewrite the scrollbar.)
- **No silent workarounds.** Do not paper over a problem with a sleep, a retry,
  a magic offset, a broad `#[allow]`, swallowing an error, or disabling a test.
  If a true fix is genuinely out of scope, say so, implement the smallest honest
  stopgap, and record it as `// TODO(<topic>):` plus a tech-debt entry — never
  present a stopgap as a fix.
- **Fix it where it belongs.** Put the change at the correct layer (the model,
  the layout engine, the patch, the build config), not wherever is easiest to
  reach. Reuse existing mechanisms instead of bolting on parallel ones.
- **Leave diagnostics out of the committed tree.** Temporary `eprintln!`/debug
  code used to localise a bug must be removed once the cause is found.
- **Verify the fix addresses the cause.** Add or update a test that would have
  caught the bug, and run `cargo check --workspace` / the relevant suite.
- **Report honestly.** If something is a workaround, partial, or unverified, say
  so plainly with the reason — do not round up to "done".

Patches and stopgaps are sometimes the *correct* tool (see
[docs/patches.md](docs/patches.md)), but only when they are deliberate,
documented, and carry a removal condition.

---

## Coding conventions

These conventions apply to all crates in the workspace.

- **File ceiling:** No `.rs` file may exceed 300 lines. Split files proactively
  before hitting the ceiling — do not write a large file and split reactively.
- **Error handling:** Use typed error enums with `thiserror`. Do not use `anyhow`
  in library crates. Do not use `.unwrap()` or `.expect()` in library code
  outside of `#[cfg(test)]` blocks.
- **Unsafe:** every crate root must carry `#![forbid(unsafe_code)]`. The sole
  exception is the three Android `cdylib` binaries (`loki-text`,
  `loki-presentation`, `loki-spreadsheet`): their `#[unsafe(no_mangle)]`
  `android_main` FFI entry point makes `forbid` impossible, so they use
  `#![deny(unsafe_code)]` + a scoped `#[allow(unsafe_code)]` (emitted by
  `loki_app_shell::android_main!`) and are enumerated in
  `scripts/unsafe-policy-allowlist.txt`. Enforced in CI by
  `scripts/check-unsafe-policy.py` (Spec 01 audit A-7). New crates must `forbid`.
- **License header:** Line 1 of every `.rs` file must be the SPDX identifier
  matching that crate's `Cargo.toml` `license` field, line 2 the copyright.
  The suite is Apache-2.0 (`// SPDX-License-Identifier: Apache-2.0`), **except
  `loki-opc`, which is MIT** (`// SPDX-License-Identifier: MIT`) because it is
  released as a standalone crate — see
  [docs/adr/0010-per-crate-licensing.md](docs/adr/0010-per-crate-licensing.md).
  Enforced in CI by `scripts/check-license-headers.py`.
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
- **Documentation Sync:** Any change to layout, rendering, or import/export properties must update the living status registry in [docs/fidelity-status.md](docs/fidelity-status.md).
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

A 2026-06-21 audit found **43 production files** over the ceiling (16 over 600
lines). The full list and a proposed split strategy live in
[docs/audit-2026-06.md](docs/audit-2026-06.md) (finding Q-1); the worst
offenders are below. This is a dedicated split-pass backlog, not a per-change
blocker — but do not *grow* these files or add new ones over the ceiling.

**The ceiling is now mechanically enforced** (Spec 01 audit A-2):
`scripts/check-file-ceiling.py` (CI) ratchets against
`scripts/file-ceiling-baseline.txt` — new files must be ≤300, baselined files
may not grow, and a file split to ≤300 must be removed from the baseline. So the
backlog can only shrink. When you split a file below the ceiling, drop its line
with `scripts/check-file-ceiling.py --update` (review the diff).

The split pass is **in progress** — current backlog is the **29** entries in the
baseline file (a 2026-07-08 pass cut ~20 files: −3600 lines across eleven new
production submodules + seven inline-test extractions, driving `doc-model`
`document.rs` and `docx/mapper/props.rs` fully under the ceiling and off the
baseline). Three techniques (the third added 2026-07-08):
1. *Inline-test extraction* (safest, no production-code change): move a file's
   `#[cfg(test)] mod tests { … }` into a sibling `<name>_tests.rs` referenced via
   `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`. Done 2026-06-21 for
   `block.rs`, `docx/mapper/{paragraph,numbering,mod,table}.rs`, `odt/import.rs`,
   `odt/mapper/lists.rs`, `layout/result.rs`, `renderer/render_layout.rs`, and
   2026-06-28 for `editing/hit_test.rs`, `xml_util.rs`, `pdf/src/page.rs`, and
   2026-07-08 for `odt/reader/styles.rs` (1554 → 1298), `odt/reader/document.rs`
   (1492 → 1002; ~490-line module), `loki-vello/scene.rs` (948 → 727),
   `loki-odf/package.rs` (644 → 410), and `loki-ooxml/docx/mapper/document.rs`
   (611 → 448) — each was over the ceiling only because of a large inline
   test module (or, as with `styles.rs`, partly so).
2. *Directory split*: convert `foo.rs` → a `foo/` directory with section-cohesive
   submodules, re-export the public entry points from `foo/mod.rs`, and move the
   tests via the same `#[path]` idiom. Give each submodule its **own explicit
   `use` list** (importing siblings via `use super::sibling::fn`) — `use super::*`
   trips `clippy::wildcard_imports`. Done for `odt/mapper/props.rs` →
   `odt/mapper/props/` and `odt/mapper/document.rs` →
   `odt/mapper/document/` (`mod`/`inlines`/`frames`/`blocks`/`page`/`meta`; worked
   examples).
3. *Cohesive-cluster extraction* (for a monolith whose tests are already
   extracted, so technique 1 doesn't apply): move a self-contained group of
   functions into a new `<name>_helper.rs` sibling declared via
   `#[path = "…"] mod <name>;`, accessing the parent's state through `super::`
   (mark the shared items `pub(super)`). The new file must itself be ≤300.
   Done 2026-07-08 for `loki-layout/src/flow.rs` (1948 → 1535, two cuts): the
   four table-geometry helpers → `flow_table_geom.rs` (`table_geom` submodule),
   and the PAGE/NUMPAGES field cluster → `flow_page_fields.rs` (`page_fields`
   submodule; a `pub(crate)` item used elsewhere is re-exported from `flow.rs`
   so its external path stays stable). Also `loki-layout/src/para.rs`
   (1856 → 1698): the tab-stop cluster → `para_tabs.rs` (`tabs` submodule); and
   `loki-layout/src/resolve.rs` (978 → 865): the `ParaProps`→`ResolvedParaProps`
   mapping → `para_props_map.rs` (`para_map` submodule). A fourth variant is
   *function-internal phase extraction* — a single >300-line function split by
   moving self-contained phases into helper fns in a sibling module (thread the
   captured locals as params; `#[allow(clippy::too_many_arguments)]` at the
   narrowest scope is acceptable, per the `flow_cell_blocks` precedent). Done
   2026-07-08 for `flow.rs`'s `flow_table` (~420 lines): row-height measurement +
   cell-decoration passes → `flow_table_paint.rs` (`table_paint` submodule),
   `flow.rs` 1535 → 1362; and for `para.rs`'s `layout_paragraph_uncached`
   (~630 lines): the two selection-geometry underlay passes (highlight fills +
   spelling squiggles) → `para_underlays.rs` (`underlays` submodule),
   `para.rs` 1698 → 1626; and `flow.rs`'s `flow_table` pass 3a (the per-cell
   content-flow loop) → `flow_table_cells.rs` (`table_cells` submodule),
   `flow.rs` 1362 → 1209 (this landed the `rotated-cell-editing` path in a
   sub-ceiling module, unblocking deferred-feature 4b.5).

(Test files are exempt from the production-line count.)

| File | Current lines | Priority |
|---|---|---|
| `loki-layout/src/para.rs` | 1626 | High |
| `loki-layout/src/flow.rs` | 1362 | High |
| `loki-ooxml/src/docx/write/document.rs` | 1073 | High |
| `loki-spreadsheet/src/routes/editor/editor_inner.rs` | 1047 | High |
| `loki-ooxml/src/docx/reader/document.rs` | 1004 | High |
| `loki-odf/src/odt/reader/styles.rs` | 892 | Med |
| `loki-layout/src/resolve.rs` | 865 | Med |
| … 22 more — see `scripts/file-ceiling-baseline.txt` (29 entries after the 2026-07-08 pass) | | |

*(Sizes above are from `scripts/file-ceiling-baseline.txt`, refreshed 2026-07-08;
the earlier numbers were stale — several files grew since first baselined.)*

(`odt/mapper/document.rs` (1094 lines) was split into the `odt/mapper/document/`
directory on 2026-06-26 — each module is now under the ceiling.)

(`read.rs` was split into `read.rs` + `props_read.rs`; both are now under 300
lines. `loro_bridge/inlines.rs` is now 219 lines, under the ceiling.
`loki-text/src/components/document_source.rs` no longer exists.)

## Known tech debt — Loro bridge round-trip gaps

Several `ParaProps`/`CharProps` fields are read from OOXML/ODF during import
but are **not perfectly round-tripped through the Loro CRDT**.

| Field(s) | Status | Priority |
|---|---|---|
| `tab_stops` | **DONE** (2026-07-04) — structured `"pos:Align:Leader;…"` codec (`loro_bridge/decode.rs`) written and read back; tested by `bridge_tab_stops_roundtrip`. Pre-fix Debug strings decode as absent. | — |
| `background_color` (paragraph) | **DONE** (2026-07-04) — total `DocumentColor` codec (`loro_bridge/color_codec.rs`, covers Rgb/Cmyk/Theme/Transparent) written and read back; tested by `bridge_para_background_color_roundtrip`. | — |
| `DocumentMeta` / `DublinCoreMeta` | Round-trips **through the Loro CRDT** (`loro_bridge::meta`) **and is written back on export** — core properties + extended Dublin Core reach DOCX (`docProps/core.xml` + `custom.xml`) and ODT (`meta.xml`), tested by `metadata_round_trip.rs` / `extended_dublin_core_round_trips`. Remaining tail (not the Loro bridge): custom user properties, `meta:editing-duration`, and OOXML `docProps/app.xml` are still not written. | Low |

---

## Known tech debt — residual vulnerable transitive `quick-xml` copies (2026-07-08)

This workspace's own `quick-xml` usage (`loki-epub`, `loki-odf`, `loki-ooxml`,
`loki-opc`) is on `0.41.0`, patched against RUSTSEC-2026-0194 (quadratic-time
duplicate-attribute check) and RUSTSEC-2026-0195 (unbounded namespace-allocation
DoS). `cargo audit` still reports both advisories against three *other*,
transitively-pulled `quick-xml` copies that this workspace does not control —
each is pinned by an intermediate crate's own manifest to a range that doesn't
yet reach `0.41`:

| Locked version | Pulled in by | Actual exposure |
|---|---|---|
| `0.38.4` | `object_store` (`^0.38.0`, via the `aws` feature — `loki-server-store`'s S3 backend) | Parses XML API responses (list-bucket, multipart) from the configured object-storage endpoint. `object_store` 0.14.0 (latest at audit time) still only requires `quick-xml ^0.40.1`, which is *also* unpatched (fix requires `>=0.41.0`) — no released `object_store` version resolves this yet. |
| `0.39.4` | `wayland-scanner` → `smithay-client-toolkit` → `winit` → `blitz-shell` (`loki-presentation`) | Build-time codegen only: generates Rust bindings from the (trusted, locally-vendored) Wayland protocol XML. Not exposed to untrusted runtime input. |
| `0.30.0` | `zbus_xml` → `zbus-lockstep` → `atspi` → `accesskit_unix` → `accesskit_winit` → `blitz-shell` | Parses AT-SPI/D-Bus introspection XML on the local session bus (Linux accessibility stack) — local IPC, not attacker-controlled document content. |

None of these are fixable by bumping our own `Cargo.toml` requirements — each is
gated behind an upstream crate release that hasn't caught up to `quick-xml`
0.41 yet. `object_store` is the one with real untrusted-network exposure and is
worth re-checking most often. Re-run `cargo audit` (or
`cargo tree -i quick-xml`) periodically and bump `object_store` /
`wayland-scanner`'s dependents the moment a release satisfies `quick-xml
>=0.41` — do not silence these via `.cargo/audit.toml` (unlike the
`rsa`/Marvin-Attack entry there, these three *are* actually compiled and
reachable).

---

## Workspace layout & capabilities

The workspace is a set of focused crates (one responsibility each). Key groups:

- **Model & bridge:** `loki-doc-model` (format-neutral `Document`, metadata,
  styles, and the Loro CRDT bridge in `loro_bridge`), `loki-primitives`,
  `loki-sheet-model`, `loki-presentation-model`.
- **Formats (one crate per family):**
  - `loki-opc` — OPC/ZIP container shared by OOXML/ODF.
  - `loki-ooxml` — DOCX/XLSX import + DOCX export.
  - `loki-odf` — ODT/ODS import + ODT/ODS export. ODT export (`odt/write/`)
    writes `content.xml` / `styles.xml` / `meta.xml`: paragraphs, headings,
    styled paragraphs, lists, tables, inline formatting, the named style
    catalog, page geometry, and metadata.
  - `loki-pdf` — **PDF/X** export (X-1a/X-3/X-4) via `pdf-writer`; reuses
    `loki-layout` for positioning, embeds fonts + images (CMYK).
  - `loki-epub` — **EPUB 3.3** export (XHTML + OCF ZIP).
- **Layout & rendering:** `loki-layout` (renderer-agnostic, Parley-based),
  `loki-vello` / `loki-renderer` / `loki-render-cache` (GPU paint; per-page
  tiles bounded by viewport virtualization).
- **Spell check:** `loki-spell` — Hunspell-compatible spell checking via the
  pure-Rust `spellbook` engine (no FFI). Tokenises text into checkable words,
  returns misspelled byte ranges + ranked suggestions; bundles a permissive
  `en` dictionary and a license-gated download catalog for other languages.
  `loki-layout` injects a checker via `LayoutOptions::spell` and emits
  `DecorationKind::Spelling` squiggles (painted by `loki-vello`). The shared
  runtime — `loki_app_shell::spell::SpellService` (locale detection, dictionary
  cache, `reqwest` fetcher) — is provided into all three apps' context;
  `loki-text` renders squiggles end-to-end via `loki_renderer::spell` ambient
  state. See §11 of `docs/fidelity-status.md` for what is wired vs. pending.
- **UI & apps:** `appthere-ui` (shared design system), `appthere-canvas`,
  `loki-i18n`, `loki-fonts`, and the binaries `loki-text` (word processor —
  the mature app), `loki-spreadsheet`, `loki-presentation`.
- **Testing:** `loki-acid` — the ACID rendering-fidelity harness (catalog of
  `TC-*` cases, embedded fixtures, page-count/glyph-coverage canaries, SSIM
  primitives, and the `load_bench` open-latency benchmark). See
  `loki-acid/README.md`.
- **Server (collaboration & storage)** — spec:
  [docs/adr/LOKI_WEB_SERVER_SPEC.md](docs/adr/LOKI_WEB_SERVER_SPEC.md)
  (ADRs C012–C020). `loki-model` (server-side IDs, `EncryptionTier`,
  RBAC `Role`/`Action`, EU-pinned `Residency`), `loki-crypto` (DEK envelope
  encryption + crypto-agile `KeyWrap`: symmetric KEK for Tiers 0/1, X25519
  for Tier-2 zero-knowledge), `loki-server-audit` (hash-chained audit log),
  `loki-server-store` (Postgres/SQLx + `object_store` ports, with in-memory
  test impls; `doc_meta.snapshot_seq` is the move-forward-only compaction
  guard), `loki-server-collab` (WebSocket relay, `FanOutBus`:
  `PgNotifyBus`/`InMemoryBus`, and the ADR-C013 `Compactor` — a periodic
  task folds each Tier-0/1 oplog backlog into a Loro snapshot and
  truncates; Tier-2 documents are compacted by clients via
  `PUT /v1/documents/{doc}/snapshot`), `loki-server-auth` (OIDC relying
  party + RBAC; keys come from the IdP's JWKS endpoint with caching and
  rotation-on-unknown-kid, or a static PEM), `loki-server-api` (REST
  `/v1`, problem+json errors, `E2eeCapabilityDisabled` = the canonical
  Tier-2 409), and the `loki-server` binary (env config with sovereignty
  validation, graceful shutdown). Deliberate deferrals are marked in-code:
  `TODO(kms)` (KEK from Vault/KMS instead of env), `TODO(headless-c025)`
  (apalis export job queue), `TODO(ws-membership)` (workspace-scope roles +
  listing join).
- **Headless (print & conversion)** — spec:
  [docs/adr/LOKI_HEADLESS_SERVER_SPEC.md](docs/adr/LOKI_HEADLESS_SERVER_SPEC.md)
  (ADRs C021–C028). `loki-convert` (ADR-C024 conversion matrix over the
  existing import/export crates — DOCX/ODT ↔ each other and → EPUB/PDF;
  XLSX ↔ ODS; PPTX/ODP/ODG gated behind the unbuilt ACID PPTX generator,
  ratified decision §5.1; unsupported pairs are a typed
  `ConversionUnsupported`), `loki-print` (ADR-C023 blocking IPP client:
  Print-Job dispatch of rendered PDF with copies/duplex/media/colour
  attributes, Get-Job-Attributes polling), and the `loki-headless` CLI
  (`convert`/`render`/`print`/`formats`; print renders non-PDF inputs via
  `loki-convert` first). The whole print path is CPU-only already
  (Parley layout → pdf-writer), so no GPU is involved. Deferrals marked
  in-code in `loki-headless/src/main.rs`: `TODO(headless-c025)` (apalis
  worker + HTTP endpoint), `TODO(headless-c021)` (vello_cpu thumbnails),
  `TODO(headless-c022)` (krilla migration for PDF/A-2b — `pdf-a2b` is a
  typed `ProfileUnsupported` today), `TODO(headless-c023-discovery)`
  (DNS-SD printer discovery), `TODO(headless-c027)` (fail-closed fonts),
  `TODO(headless-c028)` (TEE attestation).

The **Publish** ribbon tab in `loki-text` drives PDF/X + EPUB export and the
Dublin Core metadata editor.

When you add a crate, add it to the `[workspace] members` list in the root
`Cargo.toml` and give it a single clear responsibility; do not fold unrelated
concerns into an existing crate.

---

## Dependency patches & the Dioxus version pin

Some upstream crates (Dioxus Native + Blitz) are patched locally via
`[patch.crates-io]`. **Every patch is documented in
[docs/patches.md](docs/patches.md)** with its purpose, root cause, and removal
condition — keep that file in sync whenever a patch is added, changed, or
removed.

**Dioxus is pinned to an exact version (`=0.7.9`) in every crate that depends on
it.** This is deliberate, not laziness: the vendored `dioxus-native` /
`dioxus-native-dom` patches (which implement the editor's scroll-event dispatch,
`MountedData::scroll`, `onmounted`, touch, and IME support) are versioned, and a
loose `"0.7"` requirement lets Cargo resolve a newer 0.7.x from crates.io that
**silently drops the patches** (`warning: Patch ... was not used`) — which breaks
scrolling, drag, and input. Do **not** loosen the pin.

To move Dioxus to a new version, follow **"Upgrading Dioxus" in
[docs/patches.md](docs/patches.md)** (re-vendor the two patches against the new
upstream source, then bump the pin) — never just bump the version number.

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

### Conditionally-mounted panels are components (ADR-0013)

A panel shown behind a condition must be a `#[component]` mounted **at the
boundary** — `{open().then(|| rsx! { Panel { .. } })}` — never a plain function
called inside `if cond { panel(..) }` with an early `return rsx!{}`. Only a
component owns a hook scope, so only a component can call `use_breakpoint()` (or
any hook) and adapt to the size class **without threading a `compact` flag**
through the parent (which grows it). Prefer hosting the panel in
`appthere_ui::AtPanelHost`, which reads the breakpoint and picks Compact-sheet vs
Expanded-side-panel posture for you. See
[docs/adr/0013-conditional-panels-are-components.md](docs/adr/0013-conditional-panels-are-components.md).

### Confirmed CSS properties (Dioxus Native 0.7.x / Blitz)

These work in production code:

- `display: flex`, `flex-direction`, `flex-shrink`, `flex: 1`, `align-items`, `gap`
- `overflow-x: auto`, `overflow-y: auto`
- `border-radius: Npx`
- `position: relative`, `z-index: N`
- `position: absolute` (block-level) — **confirmed working** (2026-06-28). A
  block element with `position: absolute` + `top/left/right/bottom` insets,
  child of a `position: relative` parent, lays out out-of-flow at the resolved
  position, paints above in-flow siblings (and above the wgpu canvas), and
  hit-tests correctly. Verified with a runtime probe (red top/left box + green
  bottom/right box both anchored correctly; in-flow sibling not displaced). The
  floating spelling context menu (`editor_spell_panel`) relies on this. The
  earlier "unsupported" claim predated the Stylo + stylo_taffy 0.2 + Taffy 0.9
  stack and was stale. **Caveats (still unverified / known-incomplete):**
  absolute inside an *inline* formatting context (`blitz-dom`
  `layout/inline.rs` has a `TODO: Implement absolute positioning`); the
  containing block is the *immediate* positioned parent only (blitz-dom does
  not walk up to a non-immediate positioned ancestor); and `overflow: hidden`
  on the containing block clips the out-of-flow child.
- `height: calc(100vh - Npx)`, `width: 100vw`, `height: 100vh`
- `border-bottom/top: Npx solid COLOR`
- `box-sizing: border-box`

### Unconfirmed CSS properties — verify at runtime, mark with COMPAT comment

- `opacity: 0.N` (needed for disabled-state rendering)
- `white-space: nowrap` (needed for tab label truncation)
- `text-overflow: ellipsis` (needed for tab label truncation)
- `overflow-x: scroll` (explicit scroll vs. auto)
- `scrollbar-width: none` (needed for invisible scroll containers)
- `position: fixed` — collapses to `absolute` in `stylo_taffy` (not truly
  viewport-fixed); use `position: absolute` in a positioned ancestor instead.

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
   - `publish.ftl` — Publish tab: PDF/EPUB export and Dublin Core metadata
2. Use the string in code via `fl!("your-key")`.

   **Adding a whole new domain** (a new `.ftl` file) requires registering it in
   the `DOMAINS` array in `loki-i18n/src/loader.rs` — files are not
   auto-discovered.
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
