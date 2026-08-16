# Golden reference renders

> **Read this first (2026-08-16).** A working pixel gate already exists — it is
> **not** this directory. See "Where the pixel gate actually lives" below before
> producing anything here.

Drop canonical reference renders here, one PNG per page, grouped by fixture stem:

```
goldens/
  acid_docx/   page-001.png  page-002.png  …   ← Microsoft 365 desktop render
  acid_odt/    page-001.png  …                  ← LibreOffice render
  acid_xlsx/   page-001.png  …                  ← Microsoft 365 render
  acid_ods/    page-001.png  …                  ← LibreOffice render
```

Conventions (from `../TEST_PLAN.md`):

- **Authority.** OOXML (`docx`/`xlsx`/`pptx`) → Microsoft 365 desktop.
  ODF (`odt`/`ods`/`odp`/`odg`) → LibreOffice.
- **Resolution.** Render at **150 DPI**.
- **One case per page.** Keep each in-document test case on its own page so a
  failing page maps to exactly one `TC-*` id.
- **File names.** `page-001.png`, `page-002.png`, … (zero-padded, 1-based),
  matching the page order Loki produces.

The matching Loki renders go in `../renders/<stem>/page-NNN.png`. The
`golden_pixel` test pairs the two trees and diffs them with mean SSIM
(threshold 0.98). Pages present here but missing a Loki render are skipped with
a log line.

---

## Where the pixel gate actually lives

This tree is empty, and **populating it would not create a gate**, for two
reasons found while trying:

1. **`../renders/` has no in-repo producer.** The `golden_pixel` test needs
   *both* trees. Nothing in this workspace writes `renders/`; the README below
   says it is "populated by an external step". So `golden_pixel` passes in
   **0.00 s** with zero pages compared today, and would keep passing after a
   rendering regression of any size.

2. **A complete, asserting equivalent already exists.**
   `loki-render-cpu/tests/visual_golden.rs` compares Loki's deterministic
   `vello_cpu` candidate render against committed LibreOffice goldens in
   `appthere-conformance/goldens/odt/`, at a calibrated SSIM/ΔE tolerance, with
   no GPU. Three ODT fixtures, all passing. `visual_golden_docx.rs` is the same
   axis for DOCX and is waiting on Word goldens.

Prefer extending that harness over filling this one. Two golden systems for one
fact is exactly the drift this suite exists to catch.

## Why the acid fixtures are not in that harness

Measured 2026-08-16 with
`cargo run -p loki-render-cpu --example measure_odf_golden`:

| fixture | worst region | verdict |
|---|---|---|
| `styles-tinos.odt` (conformance, font-pinned) | ssim 0.6603, ΔE 7.854 | **passes** |
| `acid_odt.odt` page 1 | ssim 0.0845, ΔE 22.115 | fails |
| `acid_odt.odt` page 2 | ssim 0.2188, ΔE 14.753 | fails |

Page counts and page dimensions agree (2 pages, 1224×1584), the import is clean,
and Loki reports **no font substitutions** — so this is not a broken import and
not a missing face.

The cause is that **`acid_odt.odt` declares no `<style:default-style>`**. Its
only font declaration is on the named style `BaseBody` (Liberation Serif).
Paragraphs that neither carry a font nor inherit from `BaseBody` therefore fall
back to each *application's own* default face — Liberation Serif in LibreOffice,
a sans face in Loki. That difference repaints nearly every glyph on the page and
dominates the score, for a reason that is about the fixture, not about either
renderer.

The conformance fixtures are font-pinned by name (`para-carlito`,
`para-gelasio`, `styles-tinos`) for precisely this reason.

**To make an acid fixture gateable:** give it an explicit
`<style:default-style style:family="paragraph">` naming a bundled
metric-compatible face, regenerate its golden, and re-measure. Note that
`loki-render-cpu` also paints a grey placeholder for embedded images
(`TODO(conformance-render)`), so a fixture with images cannot reach a clean
score until that lands.

## What cannot be produced headlessly at all

`acid_docx`, `acid_xlsx` and `acid_pptx` goldens require **Microsoft 365
desktop** per the authority rule above, and Word cannot be automated headlessly.
Rendering them with LibreOffice instead would be actively wrong: every
`TC-DOCX-*` row in `../TEST_PLAN.md` names LibreOffice's divergence as the thing
being tested, so a LibreOffice "golden" would enshrine the known-wrong render as
the reference. These stay pending until captured on a Windows/macOS box via
`scripts/generate-office-goldens.sh`.

This directory is intentionally committed empty (this file only).
