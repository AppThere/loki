"""A table whose single row is taller than one page — isolated properly.

The first cut of this fixture shipped no `styles.xml`, so Word applied its
built-in Normal (1.08 line spacing) and Loki its own default (single). The
resulting 31px-vs-27px line pitch changed the pagination on its own and had
nothing to do with row splitting: it made Loki look like it was *losing* a page
when it was only fitting more lines onto each one.

This version pins everything the two sides could otherwise disagree about:

  * an explicit `docDefaults` — Carlito 11pt, single spacing, no space after;
  * per-cell `w:tcBorders` rather than table-level `w:tblBorders`, because the
    latter is only read from a *style* today (`reader/styles.rs`, gated on
    `in_style`) and a table's own `w:tblPr/w:tblBorders` is parsed nowhere —
    a separate gap that would otherwise show up here as "no borders drawn".

What is left is the question actually under test: what does each side do with a
table row that cannot fit on one page?
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

SENT = ("Sentence %d of a deliberately long cell paragraph that must run past the "
        "bottom of the page so the row cannot fit on one. ")
LONG = "".join(SENT % i for i in range(1, 60))

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


tbl = ('<w:tbl><w:tblPr><w:tblW w:w="9360" w:type="dxa"/></w:tblPr>'
       '<w:tblGrid><w:gridCol w:w="1800"/><w:gridCol w:w="7560"/></w:tblGrid>'
       '<w:tr>' + cell("LABEL", 1800) + cell(LONG, 7560) + '</w:tr>'
       '<w:tr>' + cell("AFTER", 1800) + cell("row after the tall one", 7560) + '</w:tr>'
       '</w:tbl>')

doc = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
       '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
       '<w:body>'
       '<w:p><w:r><w:t>Before the table.</w:t></w:r></w:p>'
       + tbl +
       '<w:p><w:r><w:t>After the table.</w:t></w:r></w:p>'
       '<w:sectPr>' + PG + '</w:sectPr></w:body></w:document>')

with zipfile.ZipFile("table-row-taller-than-page.docx", "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", CT)
    z.writestr("_rels/.rels", RELS)
    z.writestr("word/_rels/document.xml.rels", DOCRELS)
    z.writestr("word/styles.xml", STYLES)
    z.writestr("word/document.xml", doc)
print("built table-row-taller-than-page.docx: pinned defaults + per-cell borders")
