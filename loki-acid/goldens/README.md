# Golden reference renders

> **Read this first (2026-08-16).** A working pixel gate already exists — it is
> **not** this directory, and nothing that lands here would make it one. See
> "Where the pixel gate actually lives" and "What is committed here, and what
> it does" below before producing anything else.

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
- **Resolution.** Render at **`appthere_conformance::CONFORMANCE_DPI` (144)**.
  This line used to read "150 DPI", which appeared in no code — the rasterizer
  every path shares is pinned to 144, so that is the number with an owner.
  Goldens and candidates must agree on it or the comparison measures scaling.
- **One case per page.** Keep each in-document test case on its own page so a
  failing page maps to exactly one `TC-*` id.
- **File names.** `page-001.png`, `page-002.png`, … (zero-padded, 1-based),
  matching the page order Loki produces. The padding is load-bearing:
  discovery lists the directory and sorts by name, and `acid_docx` runs to 19
  pages, where an unpadded `page-10.png` sorts before `page-2.png`. The
  `rasterize_pdf` example emits `page-N.png`, so renaming is part of the
  procedure.

The matching Loki renders go in `../renders/<stem>/page-NNN.png`. The
`golden_pixel` test pairs the two trees and diffs them with mean SSIM
(threshold 0.98). Pages present here but missing a Loki render are skipped with
a log line.

---

## Where the pixel gate actually lives

An `acid_odt/` golden has been produced, and **it would not create a gate**, for
two reasons:

1. **`../renders/` has no in-repo producer.** The `golden_pixel` test needs
   *both* trees. Nothing in this workspace writes `renders/`; the note above
   says it is "populated by an external step". So `golden_pixel` compares
   **zero pairs** — with goldens present it logs the ones it skipped rather than
   reporting an empty set, but it stays green through a rendering regression of
   any size either way.

2. **A complete, asserting equivalent already exists.**
   `loki-render-cpu/tests/visual_golden.rs` compares Loki's deterministic
   `vello_cpu` candidate render against committed LibreOffice goldens in
   `appthere-conformance/goldens/odt/`, at a calibrated SSIM/ΔE tolerance, with
   no GPU. Three ODT fixtures, all passing. `visual_golden_docx.rs` is the same
   axis for DOCX and is waiting on Word goldens.

Prefer extending that harness over filling this one further. Two golden systems
for one fact is exactly the drift this suite exists to catch.

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

## What is committed here, and what it does

`acid_odt/GENERATION.txt` records the provenance of a real LibreOffice golden
(2 pages at 144 DPI) — the LibreOffice and rasterizer versions and the date —
mirroring the conformance tree's convention.

**The 2 page PNGs it describes are not committed.** The session that generated
them lost its git push credentials before they could be uploaded, and the
GitHub API path that landed these text files carries UTF-8 only, so binaries
could not go through it. The provenance record is committed on its own because
it is the part that cannot be reconstructed later; the PNGs can.

To supply them, either regenerate with the three commands in
`loki-render-cpu/examples/measure_odf_golden.rs` (about a minute, given
`soffice` and `poppler-utils`), or apply the `git format-patch --binary` from
that session, which was verified to restore both PNGs byte-identically. Update
`GENERATION.txt` if you regenerate — the versions in it are the ones that
produced the original.

Be clear about what landing them would and would not buy, because a PNG count is
easy to mistake for coverage:

- **`golden_pixel` would still compare nothing.** It pairs `goldens/` with
  `renders/`, and `renders/` has no in-repo producer. With goldens present the
  test names the ones it skipped instead of reporting an empty set — better
  diagnostics, not a gate.
- **Populating `renders/` would turn the suite red, not green.** `golden_pixel`
  asserts mean SSIM ≥ 0.98; `acid_odt` is nowhere near that, for the four
  itemised reasons above. The pair would fail on divergences that are
  known and partly deliberate, which is a failing build rather than a finding.
  Do not populate `renders/` until those four are resolved or the threshold is
  replaced with the calibrated `Tolerance` the conformance axis uses.
- **The golden is a moving target.** It was produced after `acid_odt.odt` was
  font-pinned, and any further fixture edit (a heading style, say) invalidates
  it.

`acid_docx` / `acid_xlsx` / `acid_pptx` have no goldens either — see below.

## What cannot be produced headlessly at all

`acid_docx`, `acid_xlsx` and `acid_pptx` goldens require **Microsoft 365
desktop** per the authority rule above, and Word cannot be automated headlessly.
Rendering them with LibreOffice instead would be actively wrong: every
`TC-DOCX-*` row in `../TEST_PLAN.md` names LibreOffice's divergence as the thing
being tested, so a LibreOffice "golden" would enshrine the known-wrong render as
the reference. These stay pending until captured on a Windows/macOS box via
`scripts/generate-office-goldens.sh`.
