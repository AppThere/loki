// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! In-memory DOCX builder for integration tests.
//!
//! [`build_reference_docx`] produces a spec-conformant DOCX ZIP exercising
//! every formatting feature that has a known fidelity gap in the audit.

use std::io::{Cursor, Write};
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;

// ── OPC namespace constants ───────────────────────────────────────────────────

const NS_PKG_RELS: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships";
const NS_W: &str =
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

// ── Part byte slices ──────────────────────────────────────────────────────────

fn content_types_xml() -> Vec<u8> {
    br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/numbering.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
  <Override PartName="/word/footnotes.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/>
  <Override PartName="/word/header1.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
  <Override PartName="/word/header2.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
  <Override PartName="/word/footer1.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>
  <Override PartName="/word/footer2.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>
</Types>"#.to_vec()
}

fn pkg_rels_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{NS_PKG_RELS}">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>"#
    ).into_bytes()
}

fn doc_rels_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{NS_PKG_RELS}">
  <Relationship Id="rId1"
    Type="{NS_R}/styles"
    Target="styles.xml"/>
  <Relationship Id="rId2"
    Type="{NS_R}/numbering"
    Target="numbering.xml"/>
  <Relationship Id="rId3"
    Type="{NS_R}/footnotes"
    Target="footnotes.xml"/>
  <Relationship Id="rId4"
    Type="{NS_R}/hyperlink"
    Target="https://example.com"
    TargetMode="External"/>
  <Relationship Id="rId5"
    Type="{NS_R}/header"
    Target="header1.xml"/>
  <Relationship Id="rId6"
    Type="{NS_R}/footer"
    Target="footer1.xml"/>
  <Relationship Id="rId7"
    Type="{NS_R}/header"
    Target="header2.xml"/>
  <Relationship Id="rId8"
    Type="{NS_R}/footer"
    Target="footer2.xml"/>
</Relationships>"#
    ).into_bytes()
}

fn styles_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="{NS_W}">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="32"/><w:color w:val="1F3864"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
    <w:pPr><w:outlineLvl w:val="1"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="26"/><w:color w:val="2E74B5"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading3">
    <w:name w:val="heading 3"/>
    <w:pPr><w:outlineLvl w:val="2"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="24"/><w:color w:val="2E74B5"/></w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="FootnoteReference">
    <w:name w:val="footnote reference"/>
    <w:rPr><w:vertAlign w:val="superscript"/></w:rPr>
  </w:style>
</w:styles>"#
    ).into_bytes()
}

fn numbering_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="{NS_W}">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0">
      <w:start w:val="1"/>
      <w:numFmt w:val="bullet"/>
      <w:lvlText w:val="&#x2022;"/>
      <w:lvlJc w:val="left"/>
      <w:pPr><w:ind w:left="360" w:hanging="360"/></w:pPr>
    </w:lvl>
    <w:lvl w:ilvl="1">
      <w:start w:val="1"/>
      <w:numFmt w:val="bullet"/>
      <w:lvlText w:val="&#x25E6;"/>
      <w:lvlJc w:val="left"/>
      <w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr>
    </w:lvl>
  </w:abstractNum>
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0">
      <w:start w:val="1"/>
      <w:numFmt w:val="decimal"/>
      <w:lvlText w:val="%1."/>
      <w:lvlJc w:val="left"/>
      <w:pPr><w:ind w:left="360" w:hanging="360"/></w:pPr>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
  <w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#
    ).into_bytes()
}

fn header_default_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="{NS_W}">
  <w:p><w:r><w:t>Test Document Header</w:t></w:r></w:p>
</w:hdr>"#
    ).into_bytes()
}

fn footer_default_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="{NS_W}">
  <w:p><w:r><w:t>Test Document Footer</w:t></w:r></w:p>
</w:ftr>"#
    ).into_bytes()
}

fn header_first_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="{NS_W}">
  <w:p><w:r><w:t>First Page Header</w:t></w:r></w:p>
</w:hdr>"#
    ).into_bytes()
}

fn footer_first_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="{NS_W}">
  <w:p><w:r><w:t>First Page Footer</w:t></w:r></w:p>
</w:ftr>"#
    ).into_bytes()
}

fn footnotes_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="{NS_W}">
  <w:footnote w:type="separator" w:id="-1">
    <w:p><w:r><w:separator/></w:r></w:p>
  </w:footnote>
  <w:footnote w:type="continuationSeparator" w:id="0">
    <w:p><w:r><w:continuationSeparator/></w:r></w:p>
  </w:footnote>
  <w:footnote w:id="1">
    <w:p>
      <w:r>
        <w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr>
        <w:footnoteRef/>
      </w:r>
      <w:r><w:t xml:space="preserve"> Footnote body text exercising gap #2.</w:t></w:r>
    </w:p>
  </w:footnote>
</w:footnotes>"#
    ).into_bytes()
}

/// Build the main `word/document.xml` exercising every audit-tracked
/// formatting gap:
///
/// | Feature                  | Gap # |
/// |--------------------------|-------|
/// | Bold / italic / combined | —     |
/// | Colour (#C00000 red)     | —     |
/// | Superscript / subscript  | #3    |
/// | Underline / strikethrough| —     |
/// | Highlight (yellow)       | #10   |
/// | Letter spacing 2 pt      | #13   |
/// | Space before / after     | —     |
/// | Left indent              | —     |
/// | Hanging indent           | #8    |
/// | Alignment variants       | —     |
/// | Lists (bullet + numbered)| #1    |
/// | Hyperlink                | #11   |
/// | Footnote reference       | #2    |
/// | Field code (PAGE)        | #4    |
/// | Paragraph border (box)   | #6    |
/// | Tab stops                | #7    |
/// | Page breaks (3 pages)    | —     |
/// | A4 page size             | —     |
fn document_xml() -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{NS_W}"
            xmlns:r="{NS_R}">
  <w:body>

    <!-- ── Page 1: Typography ──────────────────────────────────────── -->

    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Chapter One: Typography and Formatting</w:t></w:r>
    </w:p>

    <!-- bold · italic · bold-italic -->
    <w:p>
      <w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">Bold text </w:t></w:r>
      <w:r><w:t xml:space="preserve">and </w:t></w:r>
      <w:r><w:rPr><w:i/></w:rPr><w:t xml:space="preserve">italic text </w:t></w:r>
      <w:r><w:t xml:space="preserve">and </w:t></w:r>
      <w:r><w:rPr><w:b/><w:i/></w:rPr><w:t>bold-italic text</w:t></w:r>
    </w:p>

    <!-- colour #C00000 -->
    <w:p>
      <w:r><w:rPr><w:color w:val="C00000"/></w:rPr>
        <w:t xml:space="preserve">Red coloured text </w:t></w:r>
      <w:r><w:t>and normal text.</w:t></w:r>
    </w:p>

    <!-- superscript (gap #3) and subscript (gap #3) -->
    <w:p>
      <w:r><w:t xml:space="preserve">E=mc</w:t></w:r>
      <w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>2</w:t></w:r>
      <w:r><w:t xml:space="preserve">  and  H</w:t></w:r>
      <w:r><w:rPr><w:vertAlign w:val="subscript"/></w:rPr><w:t>2</w:t></w:r>
      <w:r><w:t>O</w:t></w:r>
    </w:p>

    <!-- underline · strikethrough -->
    <w:p>
      <w:r><w:rPr><w:u w:val="single"/></w:rPr>
        <w:t xml:space="preserve">Underlined text </w:t></w:r>
      <w:r><w:t xml:space="preserve">and </w:t></w:r>
      <w:r><w:rPr><w:strike/></w:rPr><w:t xml:space="preserve">struck text </w:t></w:r>
      <w:r><w:t xml:space="preserve">and </w:t></w:r>
      <w:r><w:rPr><w:dstrike/></w:rPr><w:t>double-struck text</w:t></w:r>
    </w:p>

    <!-- highlight yellow (gap #10) · letter spacing 2pt = 40 twips (gap #13) -->
    <w:p>
      <w:r><w:rPr><w:highlight w:val="yellow"/></w:rPr>
        <w:t xml:space="preserve">Highlighted text </w:t></w:r>
      <w:r><w:t xml:space="preserve">and </w:t></w:r>
      <w:r><w:rPr><w:spacing w:val="40"/></w:rPr>
        <w:t>letter-spaced text</w:t></w:r>
    </w:p>

    <!-- small-caps · all-caps -->
    <w:p>
      <w:r><w:rPr><w:smallCaps/></w:rPr>
        <w:t xml:space="preserve">Small-caps text </w:t></w:r>
      <w:r><w:t xml:space="preserve">and </w:t></w:r>
      <w:r><w:rPr><w:caps/></w:rPr><w:t>ALL-CAPS TEXT</w:t></w:r>
    </w:p>

    <!-- ── Page 1: Paragraph formatting ────────────────────────────── -->

    <w:p>
      <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
      <w:r><w:t>Chapter Two: Paragraph Formatting</w:t></w:r>
    </w:p>

    <!-- space before / after 12 pt each = 240 twips -->
    <w:p>
      <w:pPr><w:spacing w:before="240" w:after="240"/></w:pPr>
      <w:r><w:t>Explicit space-before and space-after of 12 pt each.</w:t></w:r>
    </w:p>

    <!-- left indent 0.5 inch = 720 twips -->
    <w:p>
      <w:pPr><w:ind w:left="720"/></w:pPr>
      <w:r><w:t>Left-indented paragraph at half an inch from the margin.</w:t></w:r>
    </w:p>

    <!-- hanging indent (gap #8): left=720, hanging=360 -->
    <w:p>
      <w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr>
      <w:r><w:t>Hanging-indent paragraph: the first line is at the outer margin while subsequent lines are indented by a further half-inch from the left edge.</w:t></w:r>
    </w:p>

    <!-- centre alignment -->
    <w:p>
      <w:pPr><w:jc w:val="center"/></w:pPr>
      <w:r><w:t>Centre-aligned paragraph text.</w:t></w:r>
    </w:p>

    <!-- right alignment -->
    <w:p>
      <w:pPr><w:jc w:val="right"/></w:pPr>
      <w:r><w:t>Right-aligned paragraph text.</w:t></w:r>
    </w:p>

    <!-- justified alignment -->
    <w:p>
      <w:pPr><w:jc w:val="both"/></w:pPr>
      <w:r><w:t>Justified paragraph: the text is spread across the full available width, aligning both the left and right edges on every line except the final one, which remains left-aligned.</w:t></w:r>
    </w:p>

    <!-- ── Lists (gap #1) ───────────────────────────────────────────── -->

    <w:p>
      <w:pPr><w:pStyle w:val="Heading3"/></w:pPr>
      <w:r><w:t>Lists and Special Elements</w:t></w:r>
    </w:p>

    <!-- bullet list numId=1, 3 top-level items, 2 nested -->
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>First bullet item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Second bullet item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Nested sub-item one</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Nested sub-item two</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Third bullet item</w:t></w:r>
    </w:p>

    <!-- numbered list numId=2 -->
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr></w:pPr>
      <w:r><w:t>First numbered item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr></w:pPr>
      <w:r><w:t>Second numbered item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr></w:pPr>
      <w:r><w:t>Third numbered item</w:t></w:r>
    </w:p>

    <!-- hyperlink (gap #11) -->
    <w:p>
      <w:r><w:t xml:space="preserve">Visit: </w:t></w:r>
      <w:hyperlink r:id="rId4">
        <w:r>
          <w:rPr><w:color w:val="0000FF"/><w:u w:val="single"/></w:rPr>
          <w:t>https://example.com</w:t>
        </w:r>
      </w:hyperlink>
    </w:p>

    <!-- field code PAGE (gap #4): complex field with snapshot value "1" -->
    <w:p>
      <w:r><w:t xml:space="preserve">Page </w:t></w:r>
      <w:r><w:fldChar w:fldCharType="begin"/></w:r>
      <w:r><w:instrText xml:space="preserve"> PAGE </w:instrText></w:r>
      <w:r><w:fldChar w:fldCharType="separate"/></w:r>
      <w:r><w:t>1</w:t></w:r>
      <w:r><w:fldChar w:fldCharType="end"/></w:r>
      <w:r><w:t xml:space="preserve"> of document.</w:t></w:r>
    </w:p>

    <!-- footnote reference (gap #2) -->
    <w:p>
      <w:r><w:t xml:space="preserve">This sentence has a footnote</w:t></w:r>
      <w:r>
        <w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr>
        <w:footnoteReference w:id="1"/>
      </w:r>
      <w:r><w:t xml:space="preserve"> appended to it.</w:t></w:r>
    </w:p>

    <!-- ── Paragraph border (gap #6): single-line box border, 1pt, black, 4pt space ── -->
    <w:p>
      <w:pPr>
        <w:pBdr>
          <w:top w:val="single" w:sz="8" w:color="000000" w:space="4"/>
          <w:bottom w:val="single" w:sz="8" w:color="000000" w:space="4"/>
          <w:left w:val="single" w:sz="8" w:color="000000" w:space="4"/>
          <w:right w:val="single" w:sz="8" w:color="000000" w:space="4"/>
        </w:pBdr>
      </w:pPr>
      <w:r><w:t>This paragraph has a single-line border on all four sides (gap #6).</w:t></w:r>
    </w:p>

    <!-- ── Tab stops (gap #7): explicit stops at 1 inch (1440 twips) and 3 inch (4320 twips) ── -->
    <w:p>
      <w:pPr>
        <w:tabs>
          <w:tab w:val="left" w:pos="1440"/>
          <w:tab w:val="left" w:pos="4320"/>
        </w:tabs>
      </w:pPr>
      <w:r><w:t xml:space="preserve">Column A</w:t></w:r>
      <w:r><w:tab/></w:r>
      <w:r><w:t xml:space="preserve">Column B</w:t></w:r>
      <w:r><w:tab/></w:r>
      <w:r><w:t>Column C</w:t></w:r>
    </w:p>

    <!-- ── Page break → page 2 ──────────────────────────────────────── -->
    <w:p><w:r><w:br w:type="page"/></w:r></w:p>

    <w:p>
      <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
      <w:r><w:t>Chapter Three: Second Page</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Body text on page two. This page exercises that the paginator correctly handles content that spans the second physical page of an A4 document.</w:t></w:r></w:p>
    <w:p><w:r><w:t>Another paragraph on page two to add further body content for layout verification purposes.</w:t></w:r></w:p>
    <w:p><w:r><w:t>Yet another paragraph to ensure the page is reasonably filled before the next page break.</w:t></w:r></w:p>

    <!-- ── Page break → page 3 ──────────────────────────────────────── -->
    <w:p><w:r><w:br w:type="page"/></w:r></w:p>

    <w:p>
      <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
      <w:r><w:t>Chapter Four: Third Page</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Body text on page three. At least three pages of content are required so that the paginated layout engine can be tested for multi-page rendering and page-boundary correctness.</w:t></w:r></w:p>
    <w:p><w:r><w:t>Final paragraph of the reference document.</w:t></w:r></w:p>

    <!-- ── Section properties: A4 page size + headers/footers ──────── -->
    <!-- A4: 595.28pt × 841.89pt → 11906 × 16838 twips -->
    <!-- w:titlePg enables the distinct first-page header/footer (gap #5) -->
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rId5"/>
      <w:headerReference w:type="first" r:id="rId7"/>
      <w:footerReference w:type="default" r:id="rId6"/>
      <w:footerReference w:type="first" r:id="rId8"/>
      <w:titlePg/>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"
               w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>

  </w:body>
</w:document>"#
    ).into_bytes()
}

// ── Public builder ────────────────────────────────────────────────────────────

/// Build a reference DOCX ZIP in memory covering all audit-tracked formatting
/// gaps.
///
/// The returned bytes can be passed directly to
/// `DocxImporter::run(std::io::Cursor::new(bytes))`.
pub fn build_reference_docx() -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", d).unwrap();
    zip.write_all(&content_types_xml()).unwrap();

    zip.start_file("_rels/.rels", d).unwrap();
    zip.write_all(&pkg_rels_xml()).unwrap();

    zip.start_file("word/_rels/document.xml.rels", d).unwrap();
    zip.write_all(&doc_rels_xml()).unwrap();

    zip.start_file("word/styles.xml", d).unwrap();
    zip.write_all(&styles_xml()).unwrap();

    zip.start_file("word/numbering.xml", d).unwrap();
    zip.write_all(&numbering_xml()).unwrap();

    zip.start_file("word/footnotes.xml", d).unwrap();
    zip.write_all(&footnotes_xml()).unwrap();

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(&document_xml()).unwrap();

    zip.start_file("word/header1.xml", d).unwrap();
    zip.write_all(&header_default_xml()).unwrap();

    zip.start_file("word/footer1.xml", d).unwrap();
    zip.write_all(&footer_default_xml()).unwrap();

    zip.start_file("word/header2.xml", d).unwrap();
    zip.write_all(&header_first_xml()).unwrap();

    zip.start_file("word/footer2.xml", d).unwrap();
    zip.write_all(&footer_first_xml()).unwrap();

    zip.finish().unwrap();
    buf
}
