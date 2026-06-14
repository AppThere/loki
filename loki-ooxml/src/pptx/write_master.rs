// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The slide master (`ppt/slideMasters/slideMaster1.xml`) and slide layout
//! (`ppt/slideLayouts/slideLayout1.xml`) parts.
//!
//! `PowerPoint` requires every slide to reference a layout, every layout to
//! reference a master, and the presentation to reference the master. These are
//! minimal, generic parts (empty shape trees, a standard color map, one blank
//! layout) that satisfy that requirement. Deriving real masters/layouts from
//! the document is a tracked follow-up.

const NS: &str = concat!(
    "xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" ",
    "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\""
);

// An empty (but schema-valid) shape tree shared by the master and layout.
const EMPTY_SP_TREE: &str = concat!(
    "<p:spTree>",
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>",
    "<p:grpSpPr/>",
    "</p:spTree>"
);

/// Builds `slideMaster1.xml`.
///
/// `r:id="rId1"` in the layout-id list points at the layout in this master's
/// relationships (registered by the exporter).
pub(super) fn slide_master_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <p:sldMaster {NS}>\
         <p:cSld>{EMPTY_SP_TREE}</p:cSld>\
         <p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" \
         accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" \
         accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>\
         <p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
         </p:sldMaster>"
    )
}

/// Builds `slideLayout1.xml` (a blank layout referencing the master via `.rels`).
pub(super) fn slide_layout_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <p:sldLayout {NS} type=\"blank\" preserve=\"1\">\
         <p:cSld name=\"Blank\">{EMPTY_SP_TREE}</p:cSld>\
         </p:sldLayout>"
    )
}
