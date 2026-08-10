// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Classification: the context rules (blank lines, ALL CAPS), the forced
//! markers, and the guards that keep them apart.

use super::{Element, classify};

#[test]
fn scene_headings_need_a_preceding_blank_or_start() {
    let els = classify("INT. HOUSE - DAY\n\nHe waits.\nEXT. YARD - DAY\n");
    assert_eq!(els[0], Element::SceneHeading("INT. HOUSE - DAY".into()));
    // "EXT." directly after action (no blank) is action, not a heading.
    assert_eq!(els[1], Element::Action("He waits.".into()));
    assert_eq!(els[2], Element::Action("EXT. YARD - DAY".into()));
}

#[test]
fn dialogue_blocks_split_cue_parenthetical_dialogue() {
    let els = classify("\nALEX (V.O.)\n(quietly)\nIt was you.\nAll along.\n\nHe leaves.");
    assert_eq!(
        els,
        vec![
            Element::Character("ALEX (V.O.)".into()),
            Element::Parenthetical("(quietly)".into()),
            Element::Dialogue("It was you.".into()),
            Element::Dialogue("All along.".into()),
            Element::Action("He leaves.".into()),
        ]
    );
}

#[test]
fn transitions_are_capped_to_colon_lines_between_blanks() {
    let els = classify("Beat.\n\nCUT TO:\n\nINT. LAB - NIGHT\n");
    assert_eq!(els[1], Element::Transition("CUT TO:".into()));
    assert_eq!(els[2], Element::SceneHeading("INT. LAB - NIGHT".into()));
    // An all-caps TO: line with content right after it is a cue, not a
    // transition (the next-blank guard).
    let els = classify("\nCUT TO:\nWhat?\n");
    assert_eq!(els[0], Element::Character("CUT TO:".into()));
}

#[test]
fn forced_markers_override_context() {
    let els = classify("!INT. NOT A HEADING\n.rooftop\n>FADE OUT.\n>centered<\n@McAVOY\nHi.\n");
    assert_eq!(els[0], Element::Action("INT. NOT A HEADING".into()));
    assert_eq!(els[1], Element::SceneHeading("rooftop".into()));
    assert_eq!(els[2], Element::Transition("FADE OUT.".into()));
    assert_eq!(els[3], Element::CenteredAction("centered".into()));
    assert_eq!(els[4], Element::Character("McAVOY".into()));
    assert_eq!(els[5], Element::Dialogue("Hi.".into()));
}

#[test]
fn page_break_lines_become_break_markers() {
    let els = classify("One.\n\n===\n\nTwo.");
    assert_eq!(
        els,
        vec![
            Element::Action("One.".into()),
            Element::PageBreak,
            Element::Action("Two.".into()),
        ]
    );
}

#[test]
fn all_caps_action_at_end_is_not_a_cue() {
    // A cue needs a following non-blank line; a lone caps line is action.
    let els = classify("\nSLAM!\n\nDone.");
    assert_eq!(els[0], Element::Action("SLAM!".into()));
}
