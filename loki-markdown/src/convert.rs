// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The pulldown-cmark event stream → [`Document`] converter: an inline
//! wrapper stack plus a small amount of block context (blockquote depth, the
//! list stack, the current table).
//!
//! Deliberate simplifications, chosen over silent misrendering:
//! - A list item's *extra* paragraphs (loose items) become plain paragraphs
//!   after the item — a second marker would misnumber the list.
//! - Ordered-list start offsets are dropped (the model's start value lives in
//!   the shared style, not per instance).
//! - Images keep their alt text and target but no bytes are fetched.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, LinkTarget};
use loki_doc_model::document::Document;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Which default list a list-stack frame draws its items from.
#[derive(Clone, Copy)]
enum ListFlavor {
    Bullet,
    Numbered,
}

/// The converter's whole state.
struct Builder {
    blocks: Vec<Block>,
    /// Wrapper frames for nested inline containers (emph inside link, …).
    inline_stack: Vec<(InlineWrap, Vec<Inline>)>,
    inlines: Vec<Inline>,
    /// Innermost-last list flavors; depth = len - 1 for the current item.
    list_stack: Vec<ListFlavor>,
    /// Set between Start(Item) and the item's first paragraph content.
    item_pending: bool,
    blockquote_depth: usize,
    heading: Option<u8>,
    /// In-progress code block text, with its fence info string.
    code: Option<(String, String)>,
    /// In-progress table: `(collected rows, current row, in-cell inlines)`.
    table: Option<TableBuild>,
    doc: Document,
}

struct TableBuild {
    rows: Vec<Vec<Vec<Inline>>>,
    row: Vec<Vec<Inline>>,
}

enum InlineWrap {
    Emph,
    Strong,
    Strikeout,
    Link(LinkTarget),
    Image(LinkTarget),
}

/// Converts Markdown source text into a [`Document`].
pub(crate) fn convert(text: &str) -> Document {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut b = Builder {
        blocks: Vec::new(),
        inline_stack: Vec::new(),
        inlines: Vec::new(),
        list_stack: Vec::new(),
        item_pending: false,
        blockquote_depth: 0,
        heading: None,
        code: None,
        table: None,
        doc: Document::new(),
    };

    for event in Parser::new_ext(text, options) {
        b.event(event);
    }

    b.doc.sections[0].blocks = b.blocks;
    b.doc
}

fn heading_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

impl Builder {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => match (&mut self.code, &mut self.table) {
                (Some((_, body)), _) => body.push_str(&t),
                _ => self.push_inline(Inline::Str(t.to_string())),
            },
            Event::Code(t) => {
                self.push_inline(Inline::Code(NodeAttr::default(), t.to_string()));
            }
            Event::SoftBreak => self.push_inline(Inline::SoftBreak),
            Event::HardBreak => self.push_inline(Inline::LineBreak),
            Event::Rule => self.blocks.push(Block::HorizontalRule),
            Event::TaskListMarker(done) => {
                // Rendered as text — the model has no checkbox inline.
                let mark = if done { "\u{2611} " } else { "\u{2610} " };
                self.push_inline(Inline::Str(mark.to_string()));
            }
            // HTML and other raw passthroughs are dropped (no raw rendering).
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => self.heading = Some(heading_num(level)),
            Tag::BlockQuote(_) => self.blockquote_depth += 1,
            Tag::CodeBlock(kind) => {
                let info = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(info) => info.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((info, String::new()));
            }
            Tag::List(start) => {
                // A nested list starting inside a tight item: the item's own
                // text is still pending — flush it at the *current* depth
                // before the stack deepens, or it would merge into the first
                // nested item.
                self.flush_tight_item();
                self.list_stack.push(match start {
                    Some(_) => ListFlavor::Numbered,
                    None => ListFlavor::Bullet,
                });
            }
            Tag::Item => self.item_pending = true,
            Tag::Emphasis => self.push_wrap(InlineWrap::Emph),
            Tag::Strong => self.push_wrap(InlineWrap::Strong),
            Tag::Strikethrough => self.push_wrap(InlineWrap::Strikeout),
            Tag::Link {
                dest_url, title, ..
            } => {
                let mut target = LinkTarget::new(dest_url.to_string());
                if !title.is_empty() {
                    target.title = Some(title.to_string());
                }
                self.push_wrap(InlineWrap::Link(target));
            }
            Tag::Image {
                dest_url, title, ..
            } => {
                let mut target = LinkTarget::new(dest_url.to_string());
                if !title.is_empty() {
                    target.title = Some(title.to_string());
                }
                self.push_wrap(InlineWrap::Image(target));
            }
            Tag::Table(_) => {
                self.table = Some(TableBuild {
                    rows: Vec::new(),
                    row: Vec::new(),
                });
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(t) = &mut self.table {
                    t.row = Vec::new();
                }
            }
            Tag::TableCell => self.inlines.clear(),
            // Paragraphs and everything else need no entry state.
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush_paragraph(),
            TagEnd::Heading(level) => {
                let inlines = self.take_inlines();
                self.heading = None;
                self.blocks.push(Block::Heading(
                    heading_num(level),
                    NodeAttr::default(),
                    inlines,
                ));
            }
            TagEnd::BlockQuote(_) => {
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some((info, body)) = self.code.take() {
                    let mut attr = NodeAttr::default();
                    if !info.is_empty() {
                        attr.kv.push(("language".to_string(), info));
                    }
                    // Trailing newline is the fence's, not the code's.
                    let body = body.strip_suffix('\n').unwrap_or(&body).to_string();
                    self.blocks.push(Block::CodeBlock(attr, body));
                }
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
            }
            TagEnd::Item => {
                // Tight lists have no Paragraph events — the item's inlines
                // are still pending here.
                self.flush_tight_item();
                self.item_pending = false;
            }
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Link
            | TagEnd::Image => self.pop_wrap(),
            TagEnd::TableCell => {
                let cell = self.take_inlines();
                if let Some(t) = &mut self.table {
                    t.row.push(cell);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(t) = &mut self.table {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
            }
            TagEnd::Table => self.flush_table(),
            _ => {}
        }
    }
}

// The inline-stack plumbing and block flushing, split for the 300-line
// ceiling. A child module, so `Builder`'s private fields stay private.
#[path = "convert_flush.rs"]
mod flush;
