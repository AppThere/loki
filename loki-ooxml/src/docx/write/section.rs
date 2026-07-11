// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:sectPr` serializer: page geometry, header/footer references, and
//! multi-column layout for a section.
//!
//! ECMA-376 §17.6 (sections), §17.6.13 (page size), §17.6.11 (margins),
//! §17.6.4 (columns).

use quick_xml::Writer;

use loki_doc_model::layout::page::{PageLayout, PageOrientation};

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::xml::{pts_to_twips, write_empty, write_end, write_start};

/// Writes a `<w:sectPr>` element for `layout`, registering any header/footer
/// content with `collector`.
pub(super) fn write_sect_pr<W: std::io::Write>(
    w: &mut Writer<W>,
    layout: &PageLayout,
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:sectPr", &[]);

    write_hf_refs(w, layout, collector);

    // `w:titlePg` enables the first-page header/footer for this section. It is
    // NOT what enables even-page H/F — that is the document-level
    // `w:evenAndOddHeaders` setting (written to settings.xml), so emit titlePg
    // only when a first-page variant is present.
    if layout.header_first.is_some() || layout.footer_first.is_some() {
        let _ = write_empty(w, "w:titlePg", &[]);
    }

    let pw = pts_to_twips(layout.page_size.width.value()).to_string();
    let ph = pts_to_twips(layout.page_size.height.value()).to_string();
    let orient = match layout.orientation {
        PageOrientation::Landscape => "landscape",
        PageOrientation::Portrait => "portrait",
    };
    let _ = write_empty(
        w,
        "w:pgSz",
        &[("w:w", &pw), ("w:h", &ph), ("w:orient", orient)],
    );

    let mt = pts_to_twips(layout.margins.top.value()).to_string();
    let mb = pts_to_twips(layout.margins.bottom.value()).to_string();
    let ml = pts_to_twips(layout.margins.left.value()).to_string();
    let mr = pts_to_twips(layout.margins.right.value()).to_string();
    let mh = pts_to_twips(layout.margins.header.value()).to_string();
    let mf = pts_to_twips(layout.margins.footer.value()).to_string();
    let mg = pts_to_twips(layout.margins.gutter.value()).to_string();
    let _ = write_empty(
        w,
        "w:pgMar",
        &[
            ("w:top", &mt),
            ("w:right", &mr),
            ("w:bottom", &mb),
            ("w:left", &ml),
            ("w:header", &mh),
            ("w:footer", &mf),
            ("w:gutter", &mg),
        ],
    );

    write_cols(w, layout);

    let _ = write_end(w, "w:sectPr");
}

/// Writes the `w:headerReference` / `w:footerReference` entries for every
/// present variant, registering each header/footer body with `collector`.
fn write_hf_refs<W: std::io::Write>(
    w: &mut Writer<W>,
    layout: &PageLayout,
    collector: &mut ExportCollector,
) {
    let headers = [
        ("default", &layout.header),
        ("first", &layout.header_first),
        ("even", &layout.header_even),
    ];
    for (kind, hf) in headers {
        if let Some(hf) = hf {
            let r_id = collector.add_header_footer(hf.blocks.clone(), true);
            let _ = write_empty(w, "w:headerReference", &[("w:type", kind), ("r:id", &r_id)]);
        }
    }

    let footers = [
        ("default", &layout.footer),
        ("first", &layout.footer_first),
        ("even", &layout.footer_even),
    ];
    for (kind, hf) in footers {
        if let Some(hf) = hf {
            let r_id = collector.add_header_footer(hf.blocks.clone(), false);
            let _ = write_empty(w, "w:footerReference", &[("w:type", kind), ("r:id", &r_id)]);
        }
    }
}

/// Writes `<w:cols>` for a multi-column section (ECMA-376 §17.6.4). Equal
/// columns emit a single self-closing element with `w:equalWidth="1"`; unequal
/// columns (`layout.columns.widths` present and matching `count`) emit
/// `w:equalWidth="0"` plus one `<w:col w:w=".."/>` per column. `w:sep` is
/// emitted only when a separator line is requested.
fn write_cols<W: std::io::Write>(w: &mut Writer<W>, layout: &PageLayout) {
    let Some(cols) = &layout.columns else {
        return;
    };
    let num = i32::from(cols.count).to_string();
    let space = pts_to_twips(cols.gap.value()).to_string();
    let unequal = cols.widths.len() == usize::from(cols.count);
    let mut attrs = vec![
        ("w:num", num.as_str()),
        ("w:space", space.as_str()),
        ("w:equalWidth", if unequal { "0" } else { "1" }),
    ];
    if cols.separator {
        attrs.push(("w:sep", "1"));
    }
    if !unequal {
        let _ = write_empty(w, "w:cols", &attrs);
        return;
    }
    let _ = write_start(w, "w:cols", &attrs);
    for width in &cols.widths {
        let ww = pts_to_twips(width.value()).to_string();
        let _ = write_empty(w, "w:col", &[("w:w", ww.as_str())]);
    }
    let _ = write_end(w, "w:cols");
}
