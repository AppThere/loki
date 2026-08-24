"""The *ordinary* row-split case: a row that fits on a fresh page but not here.

This is the case Loki's guard currently side-steps by moving the row whole, and
the one `iris-blueprint` page 10 exercises. It is not the same as
`table-row-taller-than-page.docx`: there the overflowing cell is simply taller
than any page, and it happens to be the row's *last* cell, so sequential cell
flow gets the right answer by accident.

Here filler text pushes the table to near the page bottom, and the row is about
half a page tall with content in **every** cell — so each cell must contribute a
fragment to both pages, which is exactly what sequential cell flow cannot do.

Defaults are pinned (see make-tallrow-fixture.py for why) and borders are
per-cell, so neither stands in for the thing under test.
"""
import zipfile

CT = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>'''

RELS = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'''

DOCRELS = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>'''

STYLES = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr>
<w:rFonts w:ascii="Carlito" w:hAnsi="Carlito" w:cs="Carlito"/><w:sz w:val="22"/>
</w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr>
<w:spacing w:after="0" w:line="240" w:lineRule="auto"/>
</w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
</w:styles>'''

PG = ('<w:pgSz w:w="12240" w:h="15840"/>'
      '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>')

BORDERS = ('<w:tcBorders>'
           '<w:top w:val="single" w:sz="8" w:color="000000"/>'
           '<w:bottom w:val="single" w:sz="8" w:color="000000"/>'
           '<w:left w:val="single" w:sz="8" w:color="000000"/>'
           '<w:right w:val="single" w:sz="8" w:color="000000"/>'
           '</w:tcBorders>')


def cell(text, w):
    return ('<w:tc><w:tcPr><w:tcW w:w="%d" w:type="dxa"/>%s</w:tcPr>'
            '<w:p><w:r><w:t xml:space="preserve">%s</w:t></w:r></w:p></w:tc>'
            % (w, BORDERS, text))


def para(t):
    return '<w:p><w:r><w:t xml:space="preserve">%s</w:t></w:r></w:p>' % t


# ~36 filler lines leaves roughly a quarter page; the row below needs about half.
filler = "".join(para("Filler line %d before the table." % i) for i in range(1, 37))

# Every cell has enough text to straddle the break.
def body_text(tag):
    return "".join("%s sentence %d that keeps this cell tall enough to straddle "
                   "the page break. " % (tag, i) for i in range(1, 12))


tbl = ('<w:tbl><w:tblPr><w:tblW w:w="9360" w:type="dxa"/></w:tblPr>'
       '<w:tblGrid><w:gridCol w:w="3120"/><w:gridCol w:w="3120"/><w:gridCol w:w="3120"/></w:tblGrid>'
       '<w:tr>' + cell(body_text("Alpha"), 3120) + cell(body_text("Beta"), 3120)
       + cell(body_text("Gamma"), 3120) + '</w:tr>'
       '<w:tr>' + cell("after A", 3120) + cell("after B", 3120) + cell("after C", 3120)
       + '</w:tr></w:tbl>')

doc = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
       '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
       '<w:body>' + filler + tbl + para("After the table.")
       + '<w:sectPr>' + PG + '</w:sectPr></w:body></w:document>')

with zipfile.ZipFile("table-row-split-at-page-break.docx", "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", CT)
    z.writestr("_rels/.rels", RELS)
    z.writestr("word/_rels/document.xml.rels", DOCRELS)
    z.writestr("word/styles.xml", STYLES)
    z.writestr("word/document.xml", doc)
print("built table-row-split-at-page-break.docx: 3 tall cells straddling a break")
