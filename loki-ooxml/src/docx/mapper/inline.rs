// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline content mapper: paragraph children → [`Vec<Inline>`].
//!
//! Implements the OOXML complex-field state machine
//! (`w:fldChar` / `w:instrText`) to assemble [`Inline::Field`] values
//! and maps runs, hyperlinks, bookmarks, and drawings.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::field::types::Field;
use loki_doc_model::content::inline::{BookmarkKind, Inline, LinkTarget, NoteKind, StyledRun};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;

use crate::docx::model::paragraph::{DocxParaChild, DocxRun, DocxRunChild};
use crate::error::{NoteKind as WarnNoteKind, OoxmlWarning};

use super::document::MappingContext;
use super::fields::{fld_simple_text, parse_field_instruction};
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
    InCode {
        instruction: String,
        depth: usize,
    },
    /// After `w:fldChar @w:fldCharType="separate"`: accumulating snapshot text.
    InResult {
        instruction: String,
        snapshot: String,
        depth: usize,
    },
}

// ── Public entry point ─────────────────────────────────────────────────────────

/// Maps a paragraph's children into a sequence of [`Inline`]s.
///
/// Implements the OOXML complex-field state machine across all children so
/// that fields that span multiple runs are assembled correctly.
pub(crate) fn map_inlines(children: &[DocxParaChild], ctx: &mut MappingContext<'_>) -> Vec<Inline> {
    let mut result: Vec<Inline> = Vec::new();
    let mut state = FieldState::Normal;

    for child in children {
        match child {
            DocxParaChild::Run(run) => {
                result.extend(process_run(run, &mut state, ctx));
            }
            DocxParaChild::Hyperlink(h) => {
                let url = if let Some(rel_id) = &h.rel_id {
                    if let Some(target) = ctx.hyperlinks.get(rel_id) {
                        target.clone()
                    } else {
                        ctx.warnings.push(OoxmlWarning::UnresolvedRelationship {
                            id: rel_id.clone(),
                            context: "hyperlink".to_string(),
                        });
                        format!("#{rel_id}")
                    }
                } else if let Some(anchor) = &h.anchor {
                    format!("#{anchor}")
                } else {
                    String::new()
                };
                let inner: Vec<Inline> = h
                    .runs
                    .iter()
                    .flat_map(|r| process_run_simple(r, ctx))
                    .collect();
                result.push(Inline::Link(
                    NodeAttr::default(),
                    inner,
                    LinkTarget { url, title: None },
                ));
            }
            DocxParaChild::BookmarkStart { id, name } => {
                // COMPAT(microsoft): w:bookmarkStart/End IDs must be unique per
                // OOXML §17.13.6.2, but programmatically generated documents
                // frequently use duplicate IDs (e.g. all bookmarks with id="1").
                // We handle this gracefully by tracking open bookmarks in a LIFO
                // stack within the MappingContext, popping the most recent matching
                // ID to resolve the bookmark name at BookmarkEnd.
                ctx.open_bookmarks.push((id.clone(), name.clone()));
                result.push(Inline::Bookmark(BookmarkKind::Start, name.clone()));
            }
            DocxParaChild::BookmarkEnd { id } => {
                let name = if let Some(pos) = ctx
                    .open_bookmarks
                    .iter()
                    .rposition(|(open_id, _)| open_id == id)
                {
                    let (_, name) = ctx.open_bookmarks.remove(pos);
                    name
                } else {
                    id.clone()
                };
                result.push(Inline::Bookmark(BookmarkKind::End, name));
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
            DocxParaChild::SimpleField { instr, runs } => {
                // `w:fldSimple` is a self-contained field: the `@w:instr`
                // instruction plus the cached result as child runs. Map it the
                // same way a complex field is, so both forms produce an
                // identical `Inline::Field`.
                let kind = parse_field_instruction(instr);
                let snapshot = fld_simple_text(runs);
                let mut field = Field::new(kind);
                let trimmed = snapshot.trim();
                if !trimmed.is_empty() {
                    field.current_value = Some(trimmed.to_string());
                }
                result.push(Inline::Field(field));
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
fn process_run(run: &DocxRun, state: &mut FieldState, ctx: &mut MappingContext<'_>) -> Vec<Inline> {
    let char_props = run.rpr.as_ref().map(map_rpr);
    let style_id = run
        .rpr
        .as_ref()
        .and_then(|rpr| rpr.style_id.as_ref())
        .map(|s| StyleId::new(s.clone()));

    let mut raw: Vec<Inline> = Vec::new();

    for child in &run.children {
        process_run_child(child, state, &mut raw, ctx);
    }

    // Wrap in StyledRun only when there is explicit formatting to preserve.
    let needs_wrap = style_id.is_some()
        || char_props
            .as_ref()
            .is_some_and(|p| p != &CharProps::default());

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
            if let FieldState::InCode { instruction, depth } = state
                && *depth == 1
            {
                instruction.push_str(text);
            }
            // depth > 1: instruction text for an inner nested field; ignored.
        }
        DocxRunChild::Text { text, .. } => match state {
            FieldState::Normal => raw.push(Inline::Str(text.clone())),
            FieldState::InResult {
                snapshot, depth, ..
            } if *depth == 1 => {
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
                    // Page and column breaks are promoted to paragraph-level flags
                    // (page_break_after / column_break_after) in map_paragraph.
                    Some("page" | "column") => {}
                    _ => raw.push(Inline::LineBreak),
                }
            }
        }
        DocxRunChild::FootnoteRef { id } => {
            if matches!(state, FieldState::Normal) {
                let blocks = lookup_note(
                    ctx.footnotes,
                    *id,
                    &mut ctx.warnings,
                    WarnNoteKind::Footnote,
                );
                raw.push(Inline::Note(NoteKind::Footnote, blocks));
            }
        }
        DocxRunChild::EndnoteRef { id } => {
            if matches!(state, FieldState::Normal) {
                let blocks =
                    lookup_note(ctx.endnotes, *id, &mut ctx.warnings, WarnNoteKind::Endnote);
                raw.push(Inline::Note(NoteKind::Endnote, blocks));
            }
        }
        DocxRunChild::Drawing(drawing) => {
            if matches!(state, FieldState::Normal)
                && let Some(img) = map_drawing(drawing, ctx)
            {
                raw.push(img);
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
                FieldState::Normal => FieldState::InCode {
                    instruction: String::new(),
                    depth: 1,
                },
                FieldState::InCode { instruction, depth } => FieldState::InCode {
                    instruction: instruction.clone(),
                    depth: depth + 1,
                },
                FieldState::InResult {
                    instruction,
                    snapshot,
                    depth,
                } => FieldState::InResult {
                    instruction: instruction.clone(),
                    snapshot: snapshot.clone(),
                    depth: depth + 1,
                },
            };
            *state = new_state;
        }
        "separate" => {
            // Only transition at depth == 1; deeper separates belong to nested fields.
            if let FieldState::InCode { instruction, depth } = &*state
                && *depth == 1
            {
                let new_state = FieldState::InResult {
                    instruction: instruction.clone(),
                    snapshot: String::new(),
                    depth: 1,
                };
                *state = new_state;
            }
        }
        "end" => {
            match state {
                FieldState::InCode { depth, .. } | FieldState::InResult { depth, .. }
                    if *depth > 1 =>
                {
                    *depth -= 1;
                }
                FieldState::InCode { instruction, .. } => {
                    let kind = parse_field_instruction(instruction);
                    raw.push(Inline::Field(Field::new(kind)));
                    *state = FieldState::Normal;
                }
                FieldState::InResult {
                    instruction,
                    snapshot,
                    ..
                } => {
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::import::DocxImportOptions;
    use crate::docx::model::paragraph::{DocxHyperlink, DocxRPr};
    use loki_doc_model::content::block::Block;
    use loki_doc_model::content::field::types::FieldKind;
    use loki_doc_model::style::catalog::StyleCatalog;
    use std::collections::HashMap;

    fn make_ctx<'a>(
        footnotes: &'a HashMap<i32, Vec<Block>>,
        endnotes: &'a HashMap<i32, Vec<Block>>,
        hyperlinks: &'a HashMap<String, String>,
        images: &'a HashMap<String, loki_opc::PartData>,
        styles: &'a StyleCatalog,
        options: &'a DocxImportOptions,
    ) -> MappingContext<'a> {
        MappingContext {
            styles,
            footnotes,
            endnotes,
            hyperlinks,
            images,
            options,
            warnings: Vec::new(),
            open_bookmarks: Vec::new(),
        }
    }

    fn plain_run(text: &str) -> DocxParaChild {
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::Text {
                text: text.to_string(),
                preserve: false,
            }],
        })
    }

    fn bold_run(text: &str) -> DocxParaChild {
        DocxParaChild::Run(DocxRun {
            rpr: Some(DocxRPr {
                bold: Some(true),
                ..Default::default()
            }),
            children: vec![DocxRunChild::Text {
                text: text.to_string(),
                preserve: false,
            }],
        })
    }

    fn default_ctx() -> (
        StyleCatalog,
        HashMap<i32, Vec<Block>>,
        HashMap<i32, Vec<Block>>,
        HashMap<String, String>,
        HashMap<String, loki_opc::PartData>,
        DocxImportOptions,
    ) {
        (
            StyleCatalog::default(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            DocxImportOptions::default(),
        )
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
        let children = vec![DocxParaChild::Hyperlink(DocxHyperlink {
            rel_id: Some("rId1".into()),
            anchor: None,
            runs: vec![DocxRun {
                rpr: None,
                children: vec![DocxRunChild::Text {
                    text: "click".into(),
                    preserve: false,
                }],
            }],
        })];
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
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "begin".into(),
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::InstrText {
                    text: " PAGE ".into(),
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "separate".into(),
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::Text {
                    text: "42".into(),
                    preserve: false,
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "end".into(),
                }],
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
    fn simple_field_maps_to_field_inline() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![DocxParaChild::SimpleField {
            instr: " PAGE ".into(),
            runs: vec![DocxRun {
                rpr: None,
                children: vec![DocxRunChild::Text {
                    text: "7".into(),
                    preserve: false,
                }],
            }],
        }];
        let inlines = map_inlines(&children, &mut ctx);
        assert_eq!(inlines.len(), 1);
        if let Inline::Field(f) = &inlines[0] {
            assert_eq!(f.kind, FieldKind::PageNumber);
            assert_eq!(f.current_value.as_deref(), Some("7"));
        } else {
            panic!("expected Field, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn empty_simple_field_has_no_current_value() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![DocxParaChild::SimpleField {
            instr: " TITLE ".into(),
            runs: vec![],
        }];
        let inlines = map_inlines(&children, &mut ctx);
        assert_eq!(inlines.len(), 1);
        if let Inline::Field(f) = &inlines[0] {
            assert_eq!(f.kind, FieldKind::Title);
            assert!(f.current_value.is_none());
        } else {
            panic!("expected Field");
        }
    }

    #[test]
    fn field_without_separate_has_no_current_value() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "begin".into(),
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::InstrText {
                    text: "TITLE".into(),
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "end".into(),
                }],
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
        let children = vec![DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FootnoteRef { id: 1 }],
        })];
        let inlines = map_inlines(&children, &mut ctx);
        assert!(
            matches!(&inlines[0], Inline::Note(NoteKind::Footnote, blocks) if !blocks.is_empty())
        );
    }

    #[test]
    fn footnote_ref_missing_emits_warning() {
        let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
        let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
        let children = vec![DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FootnoteRef { id: 99 }],
        })];
        let inlines = map_inlines(&children, &mut ctx);
        assert!(
            matches!(&inlines[0], Inline::Note(NoteKind::Footnote, blocks) if blocks.is_empty())
        );
        assert_eq!(ctx.warnings.len(), 1);
        assert!(matches!(
            &ctx.warnings[0],
            OoxmlWarning::MissingNoteContent {
                id: 99,
                kind: WarnNoteKind::Footnote
            }
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
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "begin".into(),
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::InstrText {
                    text: " IF ".into(),
                }],
            }),
            // Inner field begin (depth 2)
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "begin".into(),
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::InstrText {
                    text: " DATE ".into(),
                }],
            }),
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "end".into(),
                }],
            }),
            // Outer field end
            DocxParaChild::Run(DocxRun {
                rpr: None,
                children: vec![DocxRunChild::FldChar {
                    fld_char_type: "end".into(),
                }],
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
            DocxParaChild::BookmarkStart {
                id: "1".into(),
                name: "myBookmark".into(),
            },
            plain_run("text"),
            DocxParaChild::BookmarkEnd { id: "1".into() },
        ];
        let inlines = map_inlines(&children, &mut ctx);
        assert_eq!(inlines.len(), 3);
        assert!(
            matches!(&inlines[0], Inline::Bookmark(BookmarkKind::Start, name) if name == "myBookmark")
        );
        assert!(
            matches!(&inlines[2], Inline::Bookmark(BookmarkKind::End, name) if name == "myBookmark")
        );
    }
}
