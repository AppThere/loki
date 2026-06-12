// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Section-properties serializer for `word/document.xml`.

use quick_xml::Writer;

use loki_doc_model::layout::page::PageLayout;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::xml::{pts_to_twips, write_empty, write_end, write_start};

pub(super) fn write_sect_pr<W: std::io::Write>(
    w: &mut Writer<W>,
    layout: &PageLayout,
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:sectPr", &[]);

    if let Some(hf) = &layout.header {
        let r_id = collector.add_header_footer(hf.blocks.clone(), true);
        let _ = write_empty(
            w,
            "w:headerReference",
            &[("w:type", "default"), ("r:id", &r_id)],
        );
    }
    if let Some(hf) = &layout.header_first {
        let r_id = collector.add_header_footer(hf.blocks.clone(), true);
        let _ = write_empty(
            w,
            "w:headerReference",
            &[("w:type", "first"), ("r:id", &r_id)],
        );
    }
    if let Some(hf) = &layout.header_even {
        let r_id = collector.add_header_footer(hf.blocks.clone(), true);
        let _ = write_empty(
            w,
            "w:headerReference",
            &[("w:type", "even"), ("r:id", &r_id)],
        );
    }

    if let Some(hf) = &layout.footer {
        let r_id = collector.add_header_footer(hf.blocks.clone(), false);
        let _ = write_empty(
            w,
            "w:footerReference",
            &[("w:type", "default"), ("r:id", &r_id)],
        );
    }
    if let Some(hf) = &layout.footer_first {
        let r_id = collector.add_header_footer(hf.blocks.clone(), false);
        let _ = write_empty(
            w,
            "w:footerReference",
            &[("w:type", "first"), ("r:id", &r_id)],
        );
    }
    if let Some(hf) = &layout.footer_even {
        let r_id = collector.add_header_footer(hf.blocks.clone(), false);
        let _ = write_empty(
            w,
            "w:footerReference",
            &[("w:type", "even"), ("r:id", &r_id)],
        );
    }

    if layout.header_first.is_some()
        || layout.footer_first.is_some()
        || layout.header_even.is_some()
        || layout.footer_even.is_some()
    {
        let _ = write_empty(w, "w:titlePg", &[]);
    }

    let pw = pts_to_twips(layout.page_size.width.value()).to_string();
    let ph = pts_to_twips(layout.page_size.height.value()).to_string();
    let orient = match layout.orientation {
        loki_doc_model::layout::page::PageOrientation::Landscape => "landscape",
        loki_doc_model::layout::page::PageOrientation::Portrait => "portrait",
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

    let _ = write_end(w, "w:sectPr");
}
