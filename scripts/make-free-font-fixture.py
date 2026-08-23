#!/usr/bin/env python3
"""Rewrite a DOCX's font references to the metric-compatible free faces.

Why
---
Most of the residual differences the fidelity campaign turns up on the ACID
fixtures end in the same sentence: Loki rendered Carlito where Word rendered
Calibri, and the two disagree on some vertical metric. Advance widths match --
that is what "metric-compatible" buys -- but ascent, descent and line gap do
not, so line counts, drop-cap bands and wrap bands land a fraction of a point
apart and occasionally cross a boundary.

That is a real shipping concern, but it is *not* a layout defect, and while it
is in the picture every measurement carries it. This produces a sibling fixture
that names the free face directly, so Word and Loki rasterise the **same font
file** and any remaining difference is Loki's.

The originals stay: they are the real-world case (a document written in Word,
opened somewhere without Microsoft's fonts) and must keep being measured.

Mapping
-------
`loki_fonts`' own substitution table is the authority here -- the fixture must
ask for exactly what Loki would otherwise have substituted, or the variant
measures a different question than the original.

`Calibri Light` has no free metric-compatible Light, and `loki-fonts` maps it to
plain Carlito; this does the same. That changes how the headings *look* (regular
weight, not light) but it is precisely the substitution under test, so the
variant is faithful to what Loki will draw.

Symbol/Wingdings (bullet glyphs), `MS Mincho` (CJK) and
`Garamond Premier Pro Caption` (deliberately absent, an unresolvable-font case)
are left alone -- none is part of the metric-compatible set, and rewriting them
would remove coverage rather than a variable.

Usage
-----
    python scripts/make-free-font-fixture.py <in.docx> <out.docx>
"""

import re
import shutil
import sys
import zipfile

# loki_fonts' substitution table (see loki-fonts/src/lib.rs).
FONT_MAP = {
    "Calibri": "Carlito",
    "Calibri Light": "Carlito",
    "Cambria": "Caladea",
    "Arial": "Arimo",
    "Times New Roman": "Tinos",
    "Courier New": "Cousine",
}

# Attributes and elements that name a font family.
ATTR_RE = re.compile(r'(w:(?:ascii|hAnsi|cs|eastAsia)=")([^"]*)(")')
THEME_RE = re.compile(r'(<a:(?:latin|ea|cs) typeface=")([^"]*)(")')
FONTNAME_RE = re.compile(r'(<w:font w:name=")([^"]*)(")')


def rewrite(xml: str) -> tuple[str, int]:
    """Returns the rewritten XML and the number of names replaced."""
    n = 0

    def sub(m: re.Match) -> str:
        nonlocal n
        name = m.group(2)
        new = FONT_MAP.get(name)
        if new is None or new == name:
            return m.group(0)
        n += 1
        return m.group(1) + new + m.group(3)

    for pattern in (ATTR_RE, THEME_RE, FONTNAME_RE):
        xml = pattern.sub(sub, xml)
    return xml, n


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    src, dst = sys.argv[1], sys.argv[2]

    with zipfile.ZipFile(src) as zin:
        names = zin.namelist()
        parts = {n: zin.read(n) for n in names}
        # Preserve each entry's compression so the output stays a normal OPC zip.
        compress = {i.filename: i.compress_type for i in zin.infolist()}

    total = 0
    changed_parts = []
    for name in names:
        if not name.endswith(".xml") and not name.endswith(".rels"):
            continue
        try:
            text = parts[name].decode("utf-8")
        except UnicodeDecodeError:
            continue
        new_text, n = rewrite(text)
        if n:
            parts[name] = new_text.encode("utf-8")
            total += n
            changed_parts.append(f"{name} ({n})")

    if total == 0:
        print(f"{src}: no mapped font names found — copying unchanged")
        shutil.copyfile(src, dst)
        return 0

    with zipfile.ZipFile(dst, "w") as zout:
        for name in names:
            zout.writestr(name, parts[name], compress_type=compress.get(name, zipfile.ZIP_DEFLATED))

    print(f"{src} -> {dst}: {total} font name(s) rewritten")
    for p in changed_parts:
        print(f"    {p}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
