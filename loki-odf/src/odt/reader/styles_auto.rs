// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Automatic-styles fast reader for `content.xml`, split out of `styles.rs`
//! for the 300-line ceiling. `read_auto_styles` is re-exported `pub(crate)`
//! (called by `odt/import.rs`); `parse_style_family` is re-exported for
//! `read_stylesheet`. Style-property parsing is reached via
//! `super::parse_style_props`.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{OdfError, OdfResult};
use crate::odt::model::list_styles::OdfListStyle;
use crate::odt::model::styles::{OdfStyle, OdfStyleFamily};
use crate::xml_util::local_attr_val;

use super::{parse_list_style, parse_style_props};

/// Everything `content.xml`'s `office:automatic-styles` declares.
///
/// It carries two unrelated kinds — `style:style` and `text:list-style` — so
/// the reader returns both rather than a bare `Vec`. Returning only the former
/// is what dropped document-instance list styles on the floor; see
/// [`read_auto_styles`].
pub(crate) struct AutoStyles {
    pub styles: Vec<OdfStyle>,
    /// `text:list-style` definitions declared here rather than in `styles.xml`.
    pub list_styles: Vec<OdfListStyle>,
}

/// Parse the `office:automatic-styles` section of `content.xml` into the
/// `style:style` elements it declares (all marked `is_automatic = true`) and
/// the `text:list-style` definitions beside them.
///
/// # Why list styles are read here too
///
/// A `text:list-style` may be declared in **either** `styles.xml` or
/// `content.xml`'s automatic styles, and a `<text:list text:style-name="…">`
/// resolves against both. This reader matched only `style:style`, so a list
/// style declared in `content.xml` — which is what a writer emits for a list
/// formatted in one document rather than through a named style — was dropped
/// silently. The list then referenced a style the catalog did not contain, and
/// layout fell back to the built-in bullets: a numbered level rendered as `○`.
/// That is TC-ODT-004's visible symptom (golden `◆` / `1.`, Loki `•` / `○`).
///
/// Stops reading as soon as the closing `</office:automatic-styles>` tag is
/// encountered, so the (potentially large) `office:body` section is never
/// touched.
pub(crate) fn read_auto_styles(xml: &[u8]) -> OdfResult<AutoStyles> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut styles: Vec<OdfStyle> = Vec::new();
    let mut list_styles: Vec<OdfListStyle> = Vec::new();
    let mut inside = false;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name().into_inner().to_vec();
                match local.as_slice() {
                    b"automatic-styles" => inside = true,
                    b"list-style" if inside => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        drop(e);
                        // Same parser `styles.xml` uses — one derivation, so a
                        // level form supported there cannot be unsupported here.
                        list_styles.push(parse_list_style(&mut reader, name)?);
                    }
                    b"style" if inside => {
                        let name = local_attr_val(e, b"name").unwrap_or_default();
                        let display_name = local_attr_val(e, b"display-name");
                        let family = parse_style_family(
                            local_attr_val(e, b"family").as_deref().unwrap_or(""),
                        );
                        let parent_name = local_attr_val(e, b"parent-style-name");
                        let next_style_name = local_attr_val(e, b"next-style-name");
                        let list_style_name = local_attr_val(e, b"list-style-name");
                        let master_page_name = local_attr_val(e, b"master-page-name");
                        drop(e);
                        let props = parse_style_props(&mut reader, b"style")?;
                        styles.push(OdfStyle {
                            name,
                            display_name,
                            family,
                            parent_name,
                            next_style_name,
                            list_style_name,
                            para_props: props.para_props,
                            text_props: props.text_props,
                            col_width: props.col_width,
                            cell_props: props.cell_props,
                            graphic_wrap: props.graphic_wrap,
                            table_props: props.table_props,
                            is_automatic: true,
                            master_page_name,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                if inside && e.local_name().into_inner() == b"style" {
                    let name = local_attr_val(e, b"name").unwrap_or_default();
                    let display_name = local_attr_val(e, b"display-name");
                    let family =
                        parse_style_family(local_attr_val(e, b"family").as_deref().unwrap_or(""));
                    let parent_name = local_attr_val(e, b"parent-style-name");
                    let next_style_name = local_attr_val(e, b"next-style-name");
                    let list_style_name = local_attr_val(e, b"list-style-name");
                    let master_page_name = local_attr_val(e, b"master-page-name");
                    styles.push(OdfStyle {
                        name,
                        display_name,
                        family,
                        parent_name,
                        next_style_name,
                        list_style_name,
                        para_props: None,
                        text_props: None,
                        col_width: None,
                        cell_props: None,
                        graphic_wrap: None,
                        table_props: None,
                        is_automatic: true,
                        master_page_name,
                    });
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().into_inner() == b"automatic-styles" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OdfError::Xml {
                    part: "content.xml".to_string(),
                    source: e,
                });
            }
            _ => {}
        }
    }

    Ok(AutoStyles {
        styles,
        list_styles,
    })
}

// ── Shared helpers ─────────────────────────────────────────────────────────────

pub(super) fn parse_style_family(s: &str) -> OdfStyleFamily {
    match s {
        "paragraph" => OdfStyleFamily::Paragraph,
        "text" => OdfStyleFamily::Text,
        "table" => OdfStyleFamily::Table,
        "table-row" => OdfStyleFamily::TableRow,
        "table-cell" => OdfStyleFamily::TableCell,
        "graphic" => OdfStyleFamily::Graphic,
        _ => OdfStyleFamily::Unknown,
    }
}
