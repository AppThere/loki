// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere contributors

//! What the root sheet must say. A string test is a blunt instrument; it is the
//! right one here, because the failure it guards is a *missing* declaration and
//! the screen sitting that can see the real thing is not a unit test.

use super::ui_font_css;
use crate::tokens::typography::FONT_FAMILY_UI;

/// **The family comes from the token**, not from a copy of it. A literal here
/// would agree with the token today and drift the first time the stack changes —
/// and the drift would be invisible, because both spellings render text.
#[test]
fn the_sheet_names_the_token_and_not_a_copy_of_it() {
    let css = ui_font_css();
    assert!(
        css.contains(FONT_FAMILY_UI),
        "the sheet must carry the token verbatim: {css}",
    );
    assert!(
        FONT_FAMILY_UI.starts_with("Atkinson Hyperlegible Next"),
        "the primary face must be the bundled one, not a fallback: {FONT_FAMILY_UI}",
    );
}

/// **It must reach the document root**, which is the whole point: a rule scoped
/// to anything narrower leaves the elements mounted outside that subtree — the
/// popover host's content — inheriting the CSS initial value, which is serif.
#[test]
fn it_is_declared_on_the_document_root() {
    let css = ui_font_css();
    assert!(
        css.contains("html") && css.contains("body"),
        "a root declaration is what makes inheritance reach a root-mounted \
         overlay: {css}",
    );
}

/// **And form controls are named**, because they are the elements a UA
/// stylesheet takes back. Both halves of the sheet, so a mutation dropping
/// either one fails.
#[test]
fn form_controls_are_opted_back_into_inheritance() {
    let css = ui_font_css();
    for control in ["button", "input", "select", "textarea"] {
        assert!(
            css.contains(control),
            "{control} must be opted back in — every CSS reset does this, and a \
             menu row is a button: {css}",
        );
    }
    assert!(css.contains("inherit"), "{css}");
}
