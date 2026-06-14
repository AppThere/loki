// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Serialises the presentation part (`ppt/presentation.xml`).

use std::fmt::Write as _;

use loki_presentation_model::Presentation;

use super::units::pt_to_emu;

const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

/// Builds `ppt/presentation.xml` for `pres`.
///
/// Slide relationship ids are `rId{n}` (1-based, matching the order the
/// exporter registers them in the presentation part's `.rels`); slide ids start
/// at 257 (spec §19.2.1.7 requires id > 256). The slide master is registered
/// after the slides, so its relationship id is `rId{slide_count + 1}`.
pub(super) fn presentation_xml(pres: &Presentation) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let _ = write!(
        s,
        "<p:presentation xmlns:p=\"{NS_P}\" xmlns:r=\"{NS_R}\" xmlns:a=\"{NS_A}\">"
    );

    // Children must appear in schema order: sldMasterIdLst, sldIdLst, sldSz,
    // notesSz (ECMA-376 §19.2.1.26).
    // sldMasterId: spec requires id > 2147483648 (§19.2.1.24).
    // sldId: spec requires id > 256 (§19.2.1.7).
    let master_rid = pres.slides.len() + 1;
    let _ = write!(
        s,
        "<p:sldMasterIdLst><p:sldMasterId id=\"2147483649\" r:id=\"rId{master_rid}\"/></p:sldMasterIdLst>"
    );

    s.push_str("<p:sldIdLst>");
    for i in 0..pres.slides.len() {
        let _ = write!(s, "<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 257 + i, i + 1);
    }
    s.push_str("</p:sldIdLst>");

    let _ = write!(
        s,
        "<p:sldSz cx=\"{}\" cy=\"{}\"/>",
        pt_to_emu(pres.slide_size.width),
        pt_to_emu(pres.slide_size.height),
    );
    // Standard portrait notes size (7.5in × 10in).
    s.push_str("<p:notesSz cx=\"6858000\" cy=\"9144000\"/>");
    s.push_str("</p:presentation>");
    s
}
