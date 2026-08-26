"""Probe: how does Word resolve a table's own `w:tblBorders` against its style's?

Three tables, all referencing a style that paints a FULL grid (all six edges):

  A  style only, no direct `w:tblBorders`
     -> control for PRESENCE. If A shows no gridlines the probe cannot speak
        about the phenomenon at all (rule 3: establish it is reachable first).

  B  direct `w:tblBorders` giving ONLY the four OUTER edges, insideH/insideV
     absent (not `none` — absent).
     -> the discriminating case. Interior lines present => per-edge merge with
        the style. Interior lines absent => wholesale replacement.

  C  direct `w:tblBorders` with insideH/insideV explicitly `w:val="none"`.
     -> control for ABSENCE. Must show no interior lines under either rule; if
        it does, the probe is reading something other than the border set.

Outer edges are thick (24 eighth-points = 3pt) so outer vs interior is
unambiguous in the raster; interior lines are thin.
"""

import zipfile, pathlib

OUT = pathlib.Path(r"D:\project\loki\appthere-conformance\fixtures\docx\table-direct-borders.docx")

CT = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"""

RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"""

DOC_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"""

W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'

# A style painting a complete grid, all six edges, thin single black.
def edge(tag, sz, val="single"):
    return f'<w:{tag} w:val="{val}" w:sz="{sz}" w:space="0" w:color="000000"/>'

GRID = "".join(edge(t, 4) for t in
               ("top", "left", "bottom", "right", "insideH", "insideV"))

STYLES = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles {W}>
<w:docDefaults>
<w:rPrDefault><w:rPr>
<w:rFonts w:ascii="Arimo" w:hAnsi="Arimo" w:cs="Arimo"/>
<w:sz w:val="24"/><w:szCs w:val="24"/><w:lang w:val="en-US"/>
</w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr>
<w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/>
</w:pPr></w:pPrDefault>
</w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal">
<w:name w:val="Normal"/></w:style>
<w:style w:type="table" w:styleId="ProbeGrid">
<w:name w:val="Probe Grid"/>
<w:tblPr><w:tblBorders>{GRID}</w:tblBorders>
<w:tblCellMar>
<w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/>
<w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/>
</w:tblCellMar></w:tblPr>
</w:style>
</w:styles>"""


def cell(text, w=2400):
    return (f'<w:tc><w:tcPr><w:tcW w:w="{w}" w:type="dxa"/></w:tcPr>'
            f'<w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>')


def table(label, direct_borders):
    """A 2x2 table on style `ProbeGrid`, optionally with its own tblBorders."""
    tb = f"<w:tblBorders>{direct_borders}</w:tblBorders>" if direct_borders else ""
    rows = "".join(
        "<w:tr>" + "".join(cell(f"{label}{r}{c}") for c in (1, 2)) + "</w:tr>"
        for r in (1, 2))
    return (f'<w:tbl><w:tblPr><w:tblStyle w:val="ProbeGrid"/>'
            f'<w:tblW w:w="4800" w:type="dxa"/>'
            f'<w:tblLayout w:type="fixed"/>{tb}</w:tblPr>'
            f'<w:tblGrid><w:gridCol w:w="2400"/><w:gridCol w:w="2400"/></w:tblGrid>'
            f'{rows}</w:tbl>')


def para(t):
    return f'<w:p><w:r><w:t>{t}</w:t></w:r></w:p>'


OUTER_ONLY = "".join(edge(t, 24) for t in ("top", "left", "bottom", "right"))
OUTER_PLUS_NONE = OUTER_ONLY + edge("insideH", 0, "none") + edge("insideV", 0, "none")

body = (
    para("A: style only (control - grid must appear)")
    + table("A", None)
    + para("")
    + para("B: direct outer-only, insideH/V ABSENT (discriminating)")
    + table("B", OUTER_ONLY)
    + para("")
    + para("C: direct outer + insideH/V explicitly none (control - no grid)")
    + table("C", OUTER_PLUS_NONE)
    + '<w:sectPr><w:pgSz w:w="12240" w:h="15840"/>'
      '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"'
      ' w:header="720" w:footer="720" w:gutter="0"/></w:sectPr>'
)

DOCUMENT = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document {W}><w:body>{body}</w:body></w:document>"""

OUT.parent.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", CT)
    z.writestr("_rels/.rels", RELS)
    z.writestr("word/_rels/document.xml.rels", DOC_RELS)
    z.writestr("word/styles.xml", STYLES)
    z.writestr("word/document.xml", DOCUMENT)
print("wrote", OUT, OUT.stat().st_size, "bytes")
