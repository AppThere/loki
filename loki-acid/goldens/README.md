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
`cargo run -p loki-render-cpu --example measure_odf_golden`. `acid_odt.odt` was
**font-pinned** on the same day — it now carries a
`<style:default-style style:family="paragraph">` naming **Tinos**, and
`BaseBody` moved from `Liberation Serif` to Tinos. (Tinos is bundled in
`loki_fonts::fallback_font_blobs()`, so Loki resolves it without any system
fonts, and `scripts/generate-odf-goldens.sh`'s D4 step installs it for
fontconfig so LibreOffice shapes the same face. Liberation Serif would have made
the result depend on what happened to be installed.)

| fixture | worst region | verdict |
|---|---|---|
| `styles-tinos.odt` (conformance, font-pinned) | ssim 0.6603, ΔE 7.854 | **passes** |
| `acid_odt.odt`, before the pin | ssim 0.0845 / 0.2188 | fails |
| `acid_odt.odt`, after the pin **and** the inheritance fix below | ssim −0.0106 | fails |

The import is clean, dimensions match, and Loki reports **no font
substitutions** — this was never a broken import or a missing face.

### What the pin exposed: an ODF inheritance defect (fixed)

After pinning, styled paragraphs *still* rendered in Loki's fallback face —
specifically those whose style had no `style:parent-style-name` (`LhPct`,
`LhAtLeast`, `TabP`, `DecP`, `DropP`, `RtlP`), while unstyled content and
`ChildEmph` (parent `BaseBody`) picked up the document font. ODF 1.3 §16.2 makes
a parentless style inherit from its family's `style:default-style`;
`loki-odf`'s mapper left `parent` as `None`, and
`StyleCatalog::effective_paragraph_style` is `explicit.or(default)`, so a
paragraph *with* a style bypassed the default entirely.

Fixed in `loki-odf/src/odt/mapper/styles.rs`, with regression tests in
`styles_tests.rs`.

### What still diverges

| cause | kind |
|---|---|
| `<text:h>` carries no style; LibreOffice applies its built-in Heading 1, Loki does not | fixture under-specification |
| List markers: golden `◆` / `1.`, Loki `•` / `○` | genuine gap, TC-ODT-004 |
| Embedded image renders as a grey placeholder | harness gap, `TODO(conformance-render)` |
| Loki fits the document in 1 page, LibreOffice needs 2 | downstream of the above |

Note the page-count line. Before the pin both engines produced 2 pages — but
with *different fonts*, so the agreement was coincidental. Given the same face
they disagree, which is information the un-pinned fixture was hiding.

**Pinning further will not close the remaining three**, and pinning away
TC-ODT-004 would defeat what the fixture is for. An acid fixture is built to
diverge; that is a poor fit for a pass/fail pixel gate. For more gate coverage,
a purpose-built font-pinned fixture in the `para-carlito` / `styles-tinos`
pattern is the cheaper route.

No golden PNGs are committed for `acid_odt`: nothing reads this tree (see
above), the fixture is still moving, and a stale golden is worse than none.
Regenerate on demand with the three commands in
`loki-render-cpu/examples/measure_odf_golden.rs`.

## What cannot be produced headlessly at all

`acid_docx`, `acid_xlsx` and `acid_pptx` goldens require **Microsoft 365
desktop** per the authority rule above, and Word cannot be automated headlessly.
Rendering them with LibreOffice instead would be actively wrong: every
`TC-DOCX-*` row in `../TEST_PLAN.md` names LibreOffice's divergence as the thing
being tested, so a LibreOffice "golden" would enshrine the known-wrong render as
the reference. These stay pending until captured on a Windows/macOS box via
`scripts/generate-office-goldens.sh`.

This directory is intentionally committed empty (this file only).
