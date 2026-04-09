// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Inline content mapper: paragraph children → [`Vec<Inline>`].
//!
//! Implements the OOXML complex-field state machine
//! (`w:fldChar` / `w:instrText`) to assemble [`Inline::Field`] values
//! and maps runs, hyperlinks, bookmarks, and drawings.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};
use loki_doc_model::content::inline::{BookmarkKind, Inline, LinkTarget, NoteKind, StyledRun};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;

use crate::docx::model::paragraph::{DocxParaChild, DocxRun, DocxRunChild};
use crate::error::{NoteKind as WarnNoteKind, OoxmlWarning};

use super::document::MappingContext;
use super::images::map_drawing;
use super::props::map_rpr;

// ── Field state machine ────────────────────────────────────────────────────────

/// Assembly state for OOXML complex fields (`w:fldChar`/`w:instrText`).
///
/// `depth` tracks nested `begin` elements so that fields embedded inside
/// other field instructions are handled without panicking.
enum FieldState {
    Normal,
    /// Accumulating field instruction text. `depth ≥ 1`.
    InCode { instruction: String, depth: usize },
    /// After `w:fldChar @w:fldCharType="separate"`: accumulating snapshot text.
    InResult { instruction: String, snapshot: String, depth: usize },
}

// ── Public entry point ─────────────────────────────────────────────────────────

/// Maps a paragraph's children into a sequence of [`Inline`]s.
///
/// Implements the OOXML complex-field state machine across all children so
/// that fields that span multiple runs are assembled correctly.
pub(crate) fn map_inlines(
    children: &[DocxParaChild],
    ctx: &mut MappingContext<'_>,
) -> Vec<Inline> {
    let mut result: Vec<Inline> = Vec::new();
    let mut state = FieldState::Normal;

    for child in children {
        match child {
            DocxParaChild::Run(run) => {
                result.extend(process_run(run, &mut state, ctx));
            }
            DocxParaChild::Hyperlink(h) => {
                let url = if let Some(rel_id) = &h.rel_id {
                    ctx.hyperlinks.get(rel_id).cloned()
                        .unwrap_or_else(|| format!("#{rel_id}"))
                } else if let Some(anchor) = &h.anchor {
                    format!("#{anchor}")
                } else {
                    String::new()
                };
                let inner: Vec<Inline> = h.runs.iter()
                    .flat_map(|r| process_run_simple(r, ctx))
                    .collect();
                result.push(Inline::Link(
                    NodeAttr::default(),
                    inner,
                    LinkTarget { url, title: None },
                ));
            }
            DocxParaChild::BookmarkStart { name, .. } => {
                result.push(Inline::Bookmark(BookmarkKind::Start, name.clone()));
            }
            DocxParaChild::BookmarkEnd { id } => {
                result.push(Inline::Bookmark(BookmarkKind::End, id.clone()));
            }
            DocxParaChild::TrackDel(_) => {
                // Deleted content is skipped; it is no longer part of the document.
            }
            DocxParaChild::TrackIns(runs) => {
                // Accepted insertions are treated as normal runs.
                for run in runs {
                    result.extend(process_run(run, &mut state, ctx));
                }
            }
        }
    }

    result
}

// ── Run processing ─────────────────────────────────────────────────────────────

/// Processes a single run through the field state machine.
///
/// Returns the resulting inlines, wrapped in a [`StyledRun`] when the run
/// has an explicit character style or direct formatting properties.
fn process_run(
    run: &DocxRun,
    state: &mut FieldState,
    ctx: &mut MappingContext<'_>,
) -> Vec<Inline> {
    let char_props = run.rpr.as_ref().map(|rpr| map_rpr(rpr));
    let style_id = run.rpr.as_ref()
        .and_then(|rpr| rpr.style_id.as_ref())
        .map(|s| StyleId::new(s.clone()));

    let mut raw: Vec<Inline> = Vec::new();

    for child in &run.children {
        process_run_child(child, state, &mut raw, ctx);
    }

    // Wrap in StyledRun only when there is explicit formatting to preserve.
    let needs_wrap = style_id.is_some()
        || char_props.as_ref().map(|p| p != &CharProps::default()).unwrap_or(false);

    if needs_wrap && !raw.is_empty() {
        let styled = StyledRun {
            style_id,
            direct_props: char_props.map(Box::new),
            content: raw,
            attr: NodeAttr::default(),
        };
        vec![Inline::StyledRun(styled)]
    } else {
        raw
    }
}

/// Maps the runs in a hyperlink or similar context, using a fresh field state.
fn process_run_simple(run: &DocxRun, ctx: &mut MappingContext<'_>) -> Vec<Inline> {
    let mut state = FieldState::Normal;
    process_run(run, &mut state, ctx)
}

// ── Run child dispatch ─────────────────────────────────────────────────────────

fn process_run_child(
    child: &DocxRunChild,
    state: &mut FieldState,
    raw: &mut Vec<Inline>,
    ctx: &mut MappingContext<'_>,
) {
    match child {
        DocxRunChild::FldChar { fld_char_type } => {
            process_fld_char(fld_char_type, state, raw, ctx);
        }
        DocxRunChild::InstrText { text } => {
            if let FieldState::InCode { instruction, depth } = state {
                if *depth == 1 {
                    instruction.push_str(text);
                }
                // depth > 1: instruction text for an inner nested field; ignored.
            }
        }
        DocxRunChild::Text { text, .. } => match state {
            FieldState::Normal => raw.push(Inline::Str(text.clone())),
            FieldState::InResult { snapshot, depth, .. } if *depth == 1 => {
                snapshot.push_str(text);
            }
            _ => {} // inside field code or nested field
        },
        DocxRunChild::Tab => {
            if matches!(state, FieldState::Normal) {
                raw.push(Inline::Str("\t".to_string()));
            }
        }
        DocxRunChild::Break { break_type } => {
            if matches!(state, FieldState::Normal) {
                match break_type.as_deref() {
                    None | Some("textWrapping") => raw.push(Inline::LineBreak),
                    Some("page" | "column") => raw.push(Inline::Str("\n".to_string())),
                    _ => raw.push(Inline::LineBreak),
                }
            }
        }
        DocxRunChild::FootnoteRef { id } => {
            if matches!(state, FieldState::Normal) {
                let blocks = lookup_note(ctx.footnotes, *id, &mut ctx.warnings, WarnNoteKind::Footnote);
                raw.push(Inline::Note(NoteKind::Footnote, blocks));
            }
        }
        DocxRunChild::EndnoteRef { id } => {
            if matches!(state, FieldState::Normal) {
                let blocks = lookup_note(ctx.endnotes, *id, &mut ctx.warnings, WarnNoteKind::Endnote);
                raw.push(Inline::Note(NoteKind::Endnote, blocks));
            }
        }
        DocxRunChild::Drawing(drawing) => {
            if matches!(state, FieldState::Normal) {
                if let Some(img) = map_drawing(drawing, ctx) {
                    raw.push(img);
                }
            }
        }
    }
}

// ── Field state transitions ────────────────────────────────────────────────────

fn process_fld_char(
    fld_char_type: &str,
    state: &mut FieldState,
    raw: &mut Vec<Inline>,
    ctx: &mut MappingContext<'_>,
) {
    match fld_char_type {
        "begin" => {
            let new_state = match &*state {
                FieldState::Normal => FieldState::InCode { instruction: String::new(), depth: 1 },
                FieldState::InCode { instruction, depth } => FieldState::InCode {
                    instruction: instruction.clone(),
                    depth: depth + 1,
                },
                FieldState::InResult { instruction, snapshot, depth } => FieldState::InResult {
                    instruction: instruction.clone(),
                    snapshot: snapshot.clone(),
                    depth: depth + 1,
                },
            };
            *state = new_state;
        }
        "separate" => {
            // Only transition at depth == 1; deeper separates belong to nested fields.
            if let FieldState::InCode { instruction, depth } = &*state {
                if *depth == 1 {
                    let new_state = FieldState::InResult {
                        instruction: instruction.clone(),
                        snapshot: String::new(),
                        depth: 1,
                    };
                    *state = new_state;
                }
            }
        }
        "end" => {
            match state {
                FieldState::InCode { depth, .. } if *depth > 1 => {
                    *depth -= 1;
                }
                FieldState::InResult { depth, .. } if *depth > 1 => {
                    *depth -= 1;
                }
                FieldState::InCode { instruction, .. } => {
                    let kind = parse_field_instruction(instruction);
                    raw.push(Inline::Field(Field::new(kind)));
                    *state = FieldState::Normal;
                }
                FieldState::InResult { instruction, snapshot, .. } => {
                    let kind = parse_field_instruction(instruction);
                    let cv = snapshot.trim().to_string();
                    let mut field = Field::new(kind);
                    if !cv.is_empty() {
                        field.current_value = Some(cv);
                    }
                    raw.push(Inline::Field(field));
                    *state = FieldState::Normal;
                }
                FieldState::Normal => {
                    // Malformed: `end` with no matching `begin`.
                    ctx.warnings.push(OoxmlWarning::UnrecognisedField {
                        instruction: "<malformed:end-without-begin>".to_string(),
                    });
                }
            }
        }
        _ => {} // Unknown fldCharType; ignore.
    }
}

// ── Note lookup helper ─────────────────────────────────────────────────────────

fn lookup_note(
    map: &std::collections::HashMap<i32, Vec<Block>>,
    id: i32,
    warnings: &mut Vec<OoxmlWarning>,
    kind: WarnNoteKind,
) -> Vec<Block> {
    map.get(&id).cloned().unwrap_or_else(|| {
        warnings.push(OoxmlWarning::MissingNoteContent { id, kind });
        Vec::new()
    })
}

// ── Field instruction parser ───────────────────────────────────────────────────

/// Parses an OOXML field instruction string into a [`FieldKind`].
///
/// The first word of the instruction (case-insensitive) identifies the field
/// type. Unknown types are stored as [`FieldKind::Raw`] for round-trip
/// fidelity. ADR-0005.
fn parse_field_instruction(instruction: &str) -> FieldKind {
    let trimmed = instruction.trim();
    let first_word = trimmed.split_whitespace().next().unwrap_or("");

    match first_word.to_ascii_uppercase().as_str() {
        "PAGE" => FieldKind::PageNumber,
        "NUMPAGES" => FieldKind::PageCount,
        "DATE" => FieldKind::Date { format: extract_switch(trimmed, "@") },
        "TIME" => FieldKind::Time { format: extract_switch(trimmed, "@") },
        "TITLE" => FieldKind::Title,
        "AUTHOR" => FieldKind::Author,
        "SUBJECT" => FieldKind::Subject,
        "FILENAME" => FieldKind::FileName,
        "NUMWORDS" => FieldKind::WordCount,
        "REF" => {
            let target = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
            FieldKind::CrossReference { target, format: CrossRefFormat::Number }
        }
        "PAGEREF" => {
            let target = trimmed.split_whitespace().nth(1).unwrap_or("").to_string();
            FieldKind::CrossReference { target, format: CrossRefFormat::Page }
        }
        _ => FieldKind::Raw { instruction: trimmed.to_string() },
    }
}

/// Extracts the value following a backslash-switch (e.g. `\@`) from a field
/// instruction string.
///
/// Returns the content of the first quoted string after `\{sw}`, or `None`
/// if the switch is not present.
fn extract_switch(instruction: &str, sw: &str) -> Option<String> {
    let needle = format!("\\{sw}");
    let pos = instruction.find(&needle)?;
    let rest = instruction[pos + needle.len()..].trim_start();
    if rest.starts_with('"') {
        let end = rest[1..].find('"')?;
        Some(rest[1..=end].to_string())
    } else {
        None
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use loki_doc_model::content::block::Block;
    use loki_doc_model::style::catalog::StyleCatalog;
    use crate::docx::import::DocxImportOptions;
    use crate::docx::model::paragraph::{DocxRPr, DocxHyperlink};

    fn make_ctx<'a>(
        footnotes: &'a HashMap<i32, Vec<Block>>,
        endnotes: &'a HashMap<i32, Vec<Block>>,
        hyperlinks: &'a HashMap<String, String>,
        images: &'a HashMap<String, loki_opc::PartData>,
        styles: &'a StyleCatalog,
        options: &'a DocxImportOptions,
    ) -> MappingContext<'a> {
        MappingContext { styles, footnotes, endnotes, hyperlinks, images, options, warnings: Vec::new() }
    }

    fn plain_run(text: &str) -> DocxParaChild {
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::Text { text: text.to_string(), preserve: false }],
        })
    }

    fn bold_run(text: &str) -> DocxParaChild {
        DocxParaChild::Run(DocxRun {
            rpr: Some(DocxRPr { bold: Some(true), ..Default::default() }),
            children: vec![DocxRunChild::Text { text: text.to_string(), preserve: false }],
        })
    }

    fn default_ctx() -> (StyleCatalog, HashMap<i32, Vec<Block>>, HashMap<i32, Vec<Block>>,
                          HashMap<String, String>, HashMap<String, loki_opc::PartData>,
                          DocxImportOptions) {
        (StyleCatalog::default(), HashMap::new(), HashMap::new(),
         HashMap::new(), HashMap::new(), DocxImportOptions::default())
    }

    #[test]
    fn plain_text_run() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![plain_run("hello")];
        let inlines = map_inlines(&children, &mut ctx);
        assert_eq!(inlines, vec![Inline::Str("hello".into())]);
    }

    #[test]
    fn bold_run_produces_styled_run() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![bold_run("bold text")];
        let inlines = map_inlines(&children, &mut ctx);
        assert_eq!(inlines.len(), 1);
        if let Inline::StyledRun(sr) = &inlines[0] {
            assert_eq!(sr.direct_props.as_ref().unwrap().bold, Some(true));
            assert_eq!(sr.content, vec![Inline::Str("bold text".into())]);
        } else {
            panic!("expected StyledRun, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn hyperlink_with_url() {
        let (styles, fn_m, en_m, mut hl_m, img_m, opts) = default_ctx();
        hl_m.insert("rId1".into(), "https://example.com".into());
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![
            DocxParaChild::Hyperlink(DocxHyperlink {
                rel_id: Some("rId1".into()),
                anchor: None,
                runs: vec![DocxRun {
                    rpr: None,
                    children: vec![DocxRunChild::Text { text: "click".into(), preserve: false }],
                }],
            }),
        ];
        let inlines = map_inlines(&children, &mut ctx);
        assert_eq!(inlines.len(), 1);
        if let Inline::Link(_, content, target) = &inlines[0] {
            assert_eq!(target.url, "https://example.com");
            assert_eq!(content, &vec![Inline::Str("click".into())]);
        } else {
            panic!("expected Link");
        }
    }

    #[test]
    fn page_field_assembled() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "begin".into() }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::InstrText { text: " PAGE ".into() }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "separate".into() }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::Text { text: "42".into(), preserve: false }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "end".into() }],
            }),
        ];
        let inlines = map_inlines(&children, &mut ctx);
        assert_eq!(inlines.len(), 1);
        if let Inline::Field(f) = &inlines[0] {
            assert_eq!(f.kind, FieldKind::PageNumber);
            assert_eq!(f.current_value.as_deref(), Some("42"));
        } else {
            panic!("expected Field, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn field_without_separate_has_no_current_value() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "begin".into() }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::InstrText { text: "TITLE".into() }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "end".into() }],
            }),
        ];
        let inlines = map_inlines(&children, &mut ctx);
        if let Inline::Field(f) = &inlines[0] {
            assert_eq!(f.kind, FieldKind::Title);
            assert!(f.current_value.is_none());
        } else {
            panic!("expected Field");
        }
    }

    #[test]
    fn footnote_ref_with_content() {
        let (styles, mut fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        fn_m.insert(1, vec![Block::Para(vec![Inline::Str("note text".into())])]);
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FootnoteRef { id: 1 }],
            }),
        ];
        let inlines = map_inlines(&children, &mut ctx);
        assert!(matches!(&inlines[0], Inline::Note(NoteKind::Footnote, blocks) if !blocks.is_empty()));
    }

    #[test]
    fn footnote_ref_missing_emits_warning() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FootnoteRef { id: 99 }],
            }),
        ];
        let inlines = map_inlines(&children, &mut ctx);
        assert!(matches!(&inlines[0], Inline::Note(NoteKind::Footnote, blocks) if blocks.is_empty()));
        assert_eq!(ctx.warnings.len(), 1);
        assert!(matches!(
            &ctx.warnings[0],
            OoxmlWarning::MissingNoteContent { id: 99, kind: WarnNoteKind::Footnote }
        ));
    }

    #[test]
    fn nested_fields_do_not_panic() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        // Outer: IF { inner: DATE }
        let children = vec![
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "begin".into() }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::InstrText { text: " IF ".into() }],
            }),
            // Inner field begin (depth 2)
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "begin".into() }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::InstrText { text: " DATE ".into() }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "end".into() }],
            }),
            // Outer field end
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar { fld_char_type: "end".into() }],
            }),
        ];
        let inlines = map_inlines(&children, &mut ctx);
        // Outer IF field should be assembled (as Raw since we don't know IF)
        assert_eq!(inlines.len(), 1);
        assert!(matches!(&inlines[0], Inline::Field(_)));
    }

    #[test]
    fn bookmark_start_and_end() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![
            DocxParaChild::BookmarkStart { id: "1".into(), name: "myBookmark".into() },
            plain_run("text"),
            DocxParaChild::BookmarkEnd { id: "1".into() },
        ];
        let inlines = map_inlines(&children, &mut ctx);
        assert_eq!(inlines.len(), 3);
        assert!(matches!(&inlines[0], Inline::Bookmark(BookmarkKind::Start, name) if name == "myBookmark"));
        assert!(matches!(&inlines[2], Inline::Bookmark(BookmarkKind::End, id) if id == "1"));
    }

    #[test]
    fn parse_date_field_with_format_switch() {
        let kind = parse_field_instruction(r#" DATE \@ "MMMM d, yyyy" "#);
        assert!(matches!(kind, FieldKind::Date { format: Some(ref s) } if s == "MMMM d, yyyy"));
    }

    #[test]
    fn parse_ref_field() {
        let kind = parse_field_instruction(" REF _MyBookmark ");
        assert!(matches!(kind, FieldKind::CrossReference { target, format: CrossRefFormat::Number } if target == "_MyBookmark"));
    }

    #[test]
    fn parse_unknown_field_is_raw() {
        let kind = parse_field_instruction(" HYPERLINK \"https://example.com\" ");
        assert!(matches!(kind, FieldKind::Raw { instruction } if instruction.contains("HYPERLINK")));
    }
}
