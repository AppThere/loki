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
/// at 256 per the spec convention.
pub(super) fn presentation_xml(pres: &Presentation) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    let _ = write!(
        s,
        "<p:presentation xmlns:p=\"{NS_P}\" xmlns:r=\"{NS_R}\" xmlns:a=\"{NS_A}\">"
    );

    s.push_str("<p:sldIdLst>");
    for i in 0..pres.slides.len() {
        let _ = write!(s, "<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 256 + i, i + 1);
    }
    s.push_str("</p:sldIdLst>");

    let _ = write!(
        s,
        "<p:sldSz cx=\"{}\" cy=\"{}\"/>",
        pt_to_emu(pres.slide_size.width),
        pt_to_emu(pres.slide_size.height),
    );
    s.push_str("</p:presentation>");
    s
}
