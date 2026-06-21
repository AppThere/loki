# Golden reference renders

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

This directory is intentionally committed empty (this file only) until the
golden renders are produced from Office / LibreOffice.
