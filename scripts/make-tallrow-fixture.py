"""A table whose single row is taller than one page.

Loki's `flow_table_main` only moves a row whole when it would fit on a fresh
page (`row_max_h <= page_content_height`); a row taller than that already falls
through to the splitting path today. So this needs no code change to exercise —
it asks whether the *existing* path keeps the overflowing cell's content.

Two cells: a short label, and a long paragraph that runs well past one page.
"""
import zipfile

CT = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'''

RELS = '''<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>'''

PG = ('<w:pgSz w:w="12240" w:h="15840"/>'
      '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>')
RPR = '<w:rPr><w:rFonts w:ascii="Carlito" w:hAnsi="Carlito"/><w:sz w:val="22"/></w:rPr>'

SENT = ("Sentence %d of a deliberately long cell paragraph that must run past the "
        "bottom of the page so the row cannot fit on one. ")
LONG = "".join(SENT % i for i in range(1, 60))


def cell(text, w):
    return ('<w:tc><w:tcPr><w:tcW w:w="%d" w:type="dxa"/></w:tcPr>'
            '<w:p><w:r>%s<w:t xml:space="preserve">%s</w:t></w:r></w:p></w:tc>'
            % (w, RPR, text))


tbl = ('<w:tbl><w:tblPr><w:tblW w:w="9360" w:type="dxa"/>'
       '<w:tblBorders>'
       '<w:top w:val="single" w:sz="8" w:color="000000"/>'
       '<w:bottom w:val="single" w:sz="8" w:color="000000"/>'
       '<w:left w:val="single" w:sz="8" w:color="000000"/>'
       '<w:right w:val="single" w:sz="8" w:color="000000"/>'
       '<w:insideH w:val="single" w:sz="8" w:color="000000"/>'
       '<w:insideV w:val="single" w:sz="8" w:color="000000"/>'
       '</w:tblBorders></w:tblPr>'
       '<w:tblGrid><w:gridCol w:w="1800"/><w:gridCol w:w="7560"/></w:tblGrid>'
       '<w:tr>' + cell("LABEL", 1800) + cell(LONG, 7560) + '</w:tr>'
       '<w:tr>' + cell("AFTER", 1800) + cell("row after the tall one", 7560) + '</w:tr>'
       '</w:tbl>')

doc = ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
       '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
       '<w:body>'
       '<w:p><w:r>' + RPR + '<w:t>Before the table.</w:t></w:r></w:p>'
       + tbl +
       '<w:p><w:r>' + RPR + '<w:t>After the table.</w:t></w:r></w:p>'
       '<w:sectPr>' + PG + '</w:sectPr></w:body></w:document>')

with zipfile.ZipFile("tallrow.docx", "w", zipfile.ZIP_DEFLATED) as z:
    z.writestr("[Content_Types].xml", CT)
    z.writestr("_rels/.rels", RELS)
    z.writestr("word/document.xml", doc)
print("built tallrow.docx: one row taller than a page, plus a following row")
