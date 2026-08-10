// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Element → block emission: every element becomes a styled paragraph
//! referencing the screenplay template's style ids; page breaks and centering
//! ride direct paragraph props.

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};

use crate::classify::Element;
use crate::inline::parse_inlines;

/// Emits `elements` onto `blocks`. When `break_first` is set (a title page
/// precedes the script), the first emitted paragraph carries a page break;
/// an `===` element does the same for whatever follows it.
pub(crate) fn emit(blocks: &mut Vec<Block>, elements: &[Element], break_first: bool) {
    let mut pending_break = break_first;
    for element in elements {
        let (style, text, centered) = match element {
            Element::PageBreak => {
                pending_break = true;
                continue;
            }
            Element::SceneHeading(t) => ("SceneHeading", t, false),
            Element::Action(t) => ("Action", t, false),
            Element::CenteredAction(t) => ("Action", t, true),
            Element::Character(t) => ("Character", t, false),
            Element::Parenthetical(t) => ("Parenthetical", t, false),
            Element::Dialogue(t) => ("Dialogue", t, false),
            Element::Transition(t) => ("Transition", t, false),
        };
        let direct_para_props = (pending_break || centered).then(|| {
            Box::new(ParaProps {
                page_break_before: pending_break.then_some(true),
                alignment: centered.then_some(ParagraphAlignment::Center),
                ..Default::default()
            })
        });
        pending_break = false;
        blocks.push(Block::StyledPara(StyledParagraph {
            style_id: Some(StyleId::new(style)),
            direct_para_props,
            direct_char_props: None,
            inlines: parse_inlines(text),
            attr: Default::default(),
        }));
    }
}
