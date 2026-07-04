<!--
SPDX-License-Identifier: Apache-2.0
-->

# Spec 01 — TODO / COMPAT debt inventory (A-11)

| | |
|---|---|
| **Status** | Living inventory — regenerate from source as the tree changes |
| **Companion to** | [`spec-01-audit-report.md`](spec-01-audit-report.md) finding A-11 |
| **Snapshot** | branch `claude/adr-docs-setup-ogwz5a`, 2026-06-28 |
| **Scope** | First-party `loki-*` / `appthere-*` `src/` (production; tests excluded) |

The Spec 01 audit (A-11) flagged the in-code `TODO` / `COMPAT` debt for
cataloguing so each becomes a tracked decision rather than an invisible note.
This is that catalogue.

**Headline:** the debt is already disciplined. **All 47 production `TODO`s carry
a `TODO(<topic>)` tag** (0 bare `TODO`s) — they conform to the CLAUDE.md
convention and are enforced by `scripts/check-todo-format.py` (CI). The 57
`COMPAT` annotations are *sanctioned* workaround markers (CLAUDE.md), not debt to
remove; they are grouped here by target so each can be re-validated when its
upstream moves — most importantly against the pinned Dioxus `=0.7.9`.

Regenerate the counts below with:

```sh
git grep -hoE '//\s*TODO\([a-z0-9-]+\)' -- 'loki-*/src/**' 'appthere-*/src/**' | sort | uniq -c | sort -rn
git grep -hoE '// COMPAT\([a-z0-9-]+\)'  -- 'loki-*/src/**' 'appthere-*/src/**' | sort | uniq -c | sort -rn
```

---

## 1. COMPAT annotations (57) — grouped by target

`COMPAT` marks a deliberate workaround for an upstream limitation. Each group is
re-validated when that upstream is upgraded; the largest by far is
`dioxus-native`, which is why the Dioxus pin (`=0.7.9`, see
[`docs/patches.md`](../patches.md)) must not be loosened without re-checking
these.

| Count | Target | Re-validate when… | Notes |
|------:|--------|-------------------|-------|
| 32 | `dioxus-native` | the Dioxus `=0.7.9` pin is bumped | Blitz/Dioxus-Native CSS & API gaps; the dominant group. Re-audit on any Dioxus upgrade (patches.md "Upgrading Dioxus"). |
| 7 | `microsoft` | MS Office reference render changes | Quirks matched to Microsoft 365 (OOXML gold standard). |
| 6 | `odf` | LibreOffice reference / ODF spec | OpenDocument quirks matched to LibreOffice. |
| 3 | `android-mali` | Mali GPU driver / wgpu update | Android Mali GPU driver workarounds. |
| 1 | `android-16` | Android 16 behaviour / android-activity | The `android_main` double-fire guard (now in `loki_app_shell::android_main!`). |
| 1 | `blitz` | Blitz patch update | Blitz-specific workaround. |
| 1 | `dioxus` | Dioxus pin bump | Dioxus (non-native) workaround. |
| 1 | `loro` | Loro CRDT upgrade | Loro behaviour workaround. |
| 1 | `loro-schema` | Loro schema/version | Loro schema compatibility. |
| 1 | `loki` | internal | Internal cross-crate compatibility note. |

**Action:** none required now (these are sanctioned). The single tracked
obligation is process: when the Dioxus pin is bumped, walk the 32
`COMPAT(dioxus-native)` sites and the `dioxus`/`blitz` ones and confirm each is
still needed.

---

## 2. TODO items (47) — grouped by topic

All carry a `TODO(<topic>)` tag. Grouped by theme for triage; each is a tracked,
deferred decision, not a blocker.

### Rendering / layout fidelity (≈18)
`shadow` ×3 · `partial-render` ×2 · `odt-fidelity` ×2 · `inline-image-flow` ×2 ·
`font` ×2 · `super-sub` ×2 · `underline-style` ×1 · `strikethrough-style` ×1 ·
`floating-image` ×1 · `list-picture-bullet` ×1 · `odf-master-page` ×1 ·
`pdf-rotate` ×1 · `formatting` ×1

### UI / UX / chrome (≈17)
`ux` ×3 · `link-click` ×3 · `icons` ×3 · `a11y` ×3 · `tabs` ×2 · `theme` ×1 ·
`ribbon` ×1 · `title-edit` ×1 · `browse-templates` ×1 · `tab-default` ×1

### Editing / model (≈8)
`3b-3` ×4 · `editing` ×1 · `undo-dirty` ×1 · `loro-bridge` ×1 ·
`platform` ×2 (platform-specific paths)

**Action:** these are the product backlog expressed in code. The inventory makes
them discoverable; converting any to a tracked issue is a per-item product
decision left to the maintainer. The `TODO(<topic>)` format is now mechanically
enforced so new bare `TODO`s cannot be introduced.

---

## 3. Enforcement (new)

`scripts/check-todo-format.py` (CI) fails on any production `// TODO` /
`// FIXME` / `// HACK` / `// XXX` that is **not** in the `TODO(<topic>):` form,
keeping the inventory above meaningful: every deferred note stays greppable by
topic and can't decay into an anonymous `// TODO`. Baseline is green (all 47
already conform; 0 `HACK`/`FIXME`/`XXX`).
