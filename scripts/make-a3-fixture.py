#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 AppThere Loki contributors
"""Writes a minimal A3 DOCX, so Spec 08 R5b can be checked on a 2x display.

Why this exists
---------------
R5b — "is a permanently soft visible page an acceptable outcome of budget
pressure?" — was closed on the grounds that the policy producing it had been
deleted. It had not: L08-026 moved that behaviour from the byte target to the
survival ceiling. `plan_reachability_tests` then measured the ceiling as crossed
at plausible settings on every memory size, so soft visible text is a shipping
condition rather than a corner, and R5b needs a screen.

The obstacle is that demand goes as the square of display scale, so a 2x display
sits at 4/9 of the 3x column: US Letter peaks near 421 MiB and A4 near 435 MiB
against a 512 MiB floor on the override path, and neither reaches step 5 inside
the 4.0 zoom clamp. Consuming RAM to lower the derived ceiling does not help
either — the probe reads `MemAvailable`, which is the right figure, but
`DeviceProbeSensor` reads it in a `use_hook`, once at mount, so ballast has to be
allocated before launch and is awkward to hold steady.

Paper size is the free variable nobody was using. A3 is 2.07x Letter's area, which
puts peak demand at ~870 MiB on a 2x display — comfortably over a ceiling the
override can set. So:

    scripts/make-a3-fixture.py /tmp/a3.docx
    LOKI_TEXTURE_BUDGET_MB=520 RUST_LOG=loki_renderer=debug \\
        target/release/loki-text-desktop /tmp/a3.docx
    # zoom past 325%, then watch for survival_reduced=true

Scrolling to a page boundary is part of the procedure, not incidental. One A3
page at 400% on a 2x display is ~435 MiB — still under a 520 MiB ceiling — so the
regime is only entered at the offsets where two pages are visible at once. Zoom
alone will not do it, and a session that zoomed without scrolling would report a
clean run that never ran, which is the failure mode this whole phase keeps
meeting.

`520` and not a rounder number: an override sets the target, and the survival
ceiling is `max(target, 512 MiB)` — a person may raise the target but not lower
the ceiling, since it is about the OOM killer rather than preference. So 520 is
the smallest setting that makes ceiling and target coincide, which is what puts
step 5 within reach at 2x. Predicted first firing: 325% zoom.

What to look for, and what not to
---------------------------------
Do not judge "is it soft" by eye. R5a and R5b have *different log signatures*
and the difference is deterministic:

- **R5a (transient):** `raster_permille=750` for a page, followed within ~2 ms by
  `raster_permille=1000` for the same page. Reduced, then replaced.
- **R5b (persistent):** a reduced `raster_permille` with **no follow-up line for
  that page at all**, alongside `survival_reduced=true` on the residency line.
  The absent line is the finding.

So the observation is "did the follow-up line appear", which needs no squinting
and cannot be talked into. The subjective judgement is the separate one, and it
is a design call rather than an observation: *given* that it is persistent, is a
page held at that scale acceptable to ship?

For that judgement the scale matters, and the reachable range is wide — an
ordinary large machine at 3x is reduced to ~0.71, while A3 on 2 GiB at 4x sits at
0.271, hard against the 0.25 floor. Those are different things to look at. The
`raster_permille` on the line says which one is on screen.

The document is deliberately minimal — no styles, no numbering, no theme — so
that what is on screen is page geometry and text, and a squiggle or a layout
oddity is not competing for attention with the thing being looked at.
"""

from __future__ import annotations

import sys
import zipfile
from pathlib import Path

# A3 in twentieths of a point, the unit `w:pgSz` uses: 842pt x 1191pt.
A3_TWIPS_W = 842 * 20
A3_TWIPS_H = 1191 * 20

# Enough pages that scrolling crosses page boundaries — the offsets where two
# pages are visible at once are the ones that peak, and a one-page document
# would never reach the ceiling however far it zoomed.
PARAGRAPHS = 400

CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"""

RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"""

W_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"


def paragraph(index: int) -> str:
    text = (
        f"Paragraph {index}. This document exists to put a page-sized texture "
        "over the survival ceiling on a two-times display, which US Letter "
        "cannot do inside the four-times zoom clamp. Scroll until a page "
        "boundary crosses the viewport, then zoom past three hundred and "
        "twenty-five percent and watch whether the text on screen goes soft "
        "and stays soft."
    )
    return f'<w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>'


def document_xml() -> str:
    body = "".join(paragraph(i) for i in range(1, PARAGRAPHS + 1))
    sect = (
        f'<w:sectPr><w:pgSz w:w="{A3_TWIPS_W}" w:h="{A3_TWIPS_H}"/>'
        '<w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" '
        'w:header="0" w:footer="0" w:gutter="0"/></w:sectPr>'
    )
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:document xmlns:w="{W_NS}"><w:body>{body}{sect}</w:body></w:document>'
    )


def main(argv: list[str]) -> int:
    out = Path(argv[1] if len(argv) > 1 else "a3-residency-fixture.docx")
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CONTENT_TYPES)
        z.writestr("_rels/.rels", RELS)
        z.writestr("word/document.xml", document_xml())
    print(f"wrote {out} — A3 ({A3_TWIPS_W / 20:.0f}x{A3_TWIPS_H / 20:.0f} pt), "
          f"{PARAGRAPHS} paragraphs")
    print("\nThen, for Spec 08 R5b:")
    print(f"  LOKI_TEXTURE_BUDGET_MB=520 RUST_LOG=loki_renderer=debug \\")
    print(f"      target/release/loki-text-desktop {out}")
    print("  scroll so a page BOUNDARY is in view, then zoom past 325%")
    print("")
    print("Look for survival_reduced=true, then for a reduced raster_permille")
    print("with NO follow-up 1000 line for that page. The absent line is R5b;")
    print("a follow-up within ~2ms would be R5a. Do not judge softness by eye.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
