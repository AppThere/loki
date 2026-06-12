// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Paragraph-level serializers for `word/document.xml`.

use quick_xml::Writer;

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::Inline;

use crate::docx::write::collector::ExportCollector;
use crate::docx::write::xml::{pts_to_twips, write_empty, write_end, write_start, wval};

use super::inline::{RunProps, write_inlines, write_text_run};

/// Writes `<w:p>` with optional `w:pStyle` and optional `w:numPr`.
pub(super) fn write_para<W: std::io::Write>(
    w: &mut Writer<W>,
    style_id: Option<&str>,
    num_pr: Option<(u32, u8)>, // (numId, ilvl)
    inlines: &[Inline],
    collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:p", &[]);

    let has_ppr = style_id.is_some() || num_pr.is_some();
    if has_ppr {
        let _ = write_start(w, "w:pPr", &[]);
        if let Some(sid) = style_id {
            let _ = write_empty(w, "w:pStyle", &wval(sid));
        }
        if let Some((num_id, ilvl)) = num_pr {
            let num_id_s = num_id.to_string();
            let ilvl_s = ilvl.to_string();
            let _ = write_start(w, "w:numPr", &[]);
            let _ = write_empty(w, "w:ilvl", &wval(&ilvl_s));
            let _ = write_empty(w, "w:numId", &wval(&num_id_s));
            let _ = write_end(w, "w:numPr");
        }
        let _ = write_end(w, "w:pPr");
    }

    write_inlines(w, inlines, &RunProps::default(), collector);
    let _ = write_end(w, "w:p");
}

#[allow(clippy::similar_names)] // has_pp / has_cp / has_style — pre-existing naming
pub(super) fn write_styled_para<W: std::io::Write>(
    w: &mut Writer<W>,
    sp: &StyledParagraph,
    collector: &mut ExportCollector,
) {
    use crate::docx::write::styles::emit_char_props;

    let _ = write_start(w, "w:p", &[]);

    let has_style = sp.style_id.is_some();
    let has_pp = sp.direct_para_props.is_some();
    let has_cp = sp.direct_char_props.is_some();

    if has_style || has_pp || has_cp {
        let _ = write_start(w, "w:pPr", &[]);
        if let Some(ref sid) = sp.style_id {
            let _ = write_empty(w, "w:pStyle", &wval(sid.as_str()));
        }
        if let Some(ref pp) = sp.direct_para_props {
            // Emit para prop children inline (not a nested w:pPr).
            write_para_props_inline(w, pp);
        }
        if let Some(ref cp) = sp.direct_char_props {
            let _ = write_start(w, "w:rPr", &[]);
            emit_char_props(w, cp);
            let _ = write_end(w, "w:rPr");
        }
        let _ = write_end(w, "w:pPr");
    }

    write_inlines(w, &sp.inlines, &RunProps::default(), collector);
    let _ = write_end(w, "w:p");
}

/// Emits the children of `w:pPr` from a [`ParaProps`] (no wrapper element).
#[allow(clippy::too_many_lines, unused_assignments)] // Pre-existing pattern — structural refactor deferred
pub(super) fn write_para_props_inline<W: std::io::Write>(
    w: &mut Writer<W>,
    pp: &loki_doc_model::style::props::para_props::ParaProps,
) {
    use loki_doc_model::style::props::para_props::ParagraphAlignment;

    if let Some(align) = pp.alignment {
        let jc = match align {
            ParagraphAlignment::Right => "right",
            ParagraphAlignment::Center => "center",
            ParagraphAlignment::Justify => "both",
            _ => "left",
        };
        let _ = write_empty(w, "w:jc", &wval(jc));
    }

    let has_ind = pp.indent_start.is_some()
        || pp.indent_end.is_some()
        || pp.indent_hanging.is_some()
        || pp.indent_first_line.is_some();
    if has_ind {
        let left = pp
            .indent_start
            .map_or(0, |v| pts_to_twips(v.value()))
            .to_string();
        let right = pp
            .indent_end
            .map_or(0, |v| pts_to_twips(v.value()))
            .to_string();
        let hanging = pp
            .indent_hanging
            .map_or(0, |v| pts_to_twips(v.value()))
            .to_string();
        let first_line = pp
            .indent_first_line
            .map_or(0, |v| pts_to_twips(v.value()))
            .to_string();
        let mut attrs: Vec<(&str, &str)> = Vec::new();
        if pp.indent_start.is_some() {
            attrs.push(("w:left", &left));
        }
        if pp.indent_end.is_some() {
            attrs.push(("w:right", &right));
        }
        if pp.indent_hanging.is_some() {
            attrs.push(("w:hanging", &hanging));
        }
        if pp.indent_first_line.is_some() {
            attrs.push(("w:firstLine", &first_line));
        }
        if !attrs.is_empty() {
            let _ = write_empty(w, "w:ind", &attrs);
        }
    }

    if let Some(tabs) = &pp.tab_stops {
        let _ = write_start(w, "w:tabs", &[]);
        for ts in tabs {
            use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader};
            let val = match ts.alignment {
                TabAlignment::Center => "center",
                TabAlignment::Right => "right",
                TabAlignment::Decimal => "decimal",
                TabAlignment::Clear => "clear",
                _ => "left",
            };
            let pos = pts_to_twips(ts.position.value()).to_string();
            let leader = match ts.leader {
                TabLeader::Dot => "dot",
                TabLeader::Dash => "dash",
                TabLeader::Underscore => "underscore",
                TabLeader::Heavy => "heavy",
                TabLeader::MiddleDot => "middleDot",
                _ => "none",
            };
            let _ = write_empty(
                w,
                "w:tab",
                &[("w:val", val), ("w:pos", &pos), ("w:leader", leader)],
            );
        }
        let _ = write_end(w, "w:tabs");
    }

    if pp.space_before.is_some() || pp.space_after.is_some() || pp.line_height.is_some() {
        use loki_doc_model::style::props::para_props::{LineHeight, Spacing};
        let mut attrs: Vec<(&str, &str)> = Vec::new();

        #[allow(clippy::match_same_arms)] // Spacing is #[non_exhaustive]; wildcard required
        let before = pp.space_before.map(|v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        });
        let before_s;
        if let Some(b) = before {
            before_s = b.to_string();
            attrs.push(("w:before", &before_s));
        }

        let after = pp.space_after.map(|v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        });
        let after_s;
        if let Some(a) = after {
            after_s = a.to_string();
            attrs.push(("w:after", &after_s));
        }

        let mut line_s = String::new();
        if let Some(lh) = pp.line_height {
            match lh {
                LineHeight::Exact(pt) => {
                    line_s = pts_to_twips(pt.value()).to_string();
                    attrs.push(("w:line", &line_s));
                    attrs.push(("w:lineRule", "exact"));
                }
                LineHeight::AtLeast(pt) => {
                    line_s = pts_to_twips(pt.value()).to_string();
                    attrs.push(("w:line", &line_s));
                    attrs.push(("w:lineRule", "atLeast"));
                }
                LineHeight::Multiple(f) => {
                    line_s = (f * 2.4).round().to_string();
                    attrs.push(("w:line", &line_s));
                    attrs.push(("w:lineRule", "auto"));
                }
                _ => {}
            }
        }

        if !attrs.is_empty() {
            let _ = write_empty(w, "w:spacing", &attrs);
        }
    }
}

pub(super) fn write_code_block<W: std::io::Write>(
    w: &mut Writer<W>,
    code: &str,
    _collector: &mut ExportCollector,
) {
    let _ = write_start(w, "w:p", &[]);
    let _ = write_start(w, "w:pPr", &[]);
    let _ = write_empty(w, "w:pStyle", &wval("Code"));
    let _ = write_end(w, "w:pPr");
    write_text_run(
        w,
        code,
        &RunProps {
            code: true,
            ..Default::default()
        },
    );
    let _ = write_end(w, "w:p");
}

pub(super) fn write_horizontal_rule<W: std::io::Write>(w: &mut Writer<W>) {
    let _ = write_start(w, "w:p", &[]);
    let _ = write_start(w, "w:pPr", &[]);
    let _ = write_start(w, "w:pBdr", &[]);
    let _ = write_empty(
        w,
        "w:bottom",
        &[
            ("w:val", "single"),
            ("w:sz", "6"),
            ("w:space", "1"),
            ("w:color", "auto"),
        ],
    );
    let _ = write_end(w, "w:pBdr");
    let _ = write_end(w, "w:pPr");
    let _ = write_end(w, "w:p");
}

pub(super) fn write_line_block<W: std::io::Write>(
    w: &mut Writer<W>,
    lines: &[Vec<Inline>],
    collector: &mut ExportCollector,
) {
    for (idx, line) in lines.iter().enumerate() {
        let _ = write_start(w, "w:p", &[]);
        write_inlines(w, line, &RunProps::default(), collector);
        if idx + 1 < lines.len() {
            // Add a line break run between lines (last line gets its own paragraph).
            let _ = write_start(w, "w:r", &[]);
            let _ = write_empty(w, "w:br", &[]);
            let _ = write_end(w, "w:r");
        }
        let _ = write_end(w, "w:p");
    }
}
