// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The Browse-templates filter has to agree with the predicate that decides
//! what to do with what it returns.

use super::{MIME_TYPES, TEMPLATE_MIME_TYPES};
use crate::routes::home_util::is_template_name;

/// The extension each template MIME type stands for, in the same order.
const TEMPLATE_EXTENSIONS: &[&str] = &["dotx", "dotm", "ott"];

/// **Everything the dialog offers must be something the open path treats as a
/// template.** The Browse card opens its result as a detached copy; a filter
/// that admitted a type `is_template_name` rejects would silently open that
/// file *in place*, so the next Save would overwrite the user's template.
#[test]
fn every_offered_type_is_one_the_open_path_calls_a_template() {
    assert_eq!(
        TEMPLATE_MIME_TYPES.len(),
        TEMPLATE_EXTENSIONS.len(),
        "a MIME type was added without the extension it stands for"
    );
    for ext in TEMPLATE_EXTENSIONS {
        assert!(
            is_template_name(&format!("Report.{ext}")),
            ".{ext} is offered by the template filter but is not a template to the open path"
        );
    }
}

/// The inverse: an ordinary document must not be reachable through the template
/// dialog. Without this the test above passes for a filter that offers
/// everything.
#[test]
fn ordinary_documents_are_not_offered() {
    for ext in ["docx", "odt", "txt", "pdf"] {
        assert!(
            !is_template_name(&format!("Report.{ext}")),
            ".{ext} should not be a template"
        );
    }
    let offered: Vec<&str> = TEMPLATE_MIME_TYPES.to_vec();
    for plain in [
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.oasis.opendocument.text",
    ] {
        assert!(
            !offered.contains(&plain),
            "{plain} is a plain document type and must not be in the template filter"
        );
    }
}

/// The template filter is a **subset** of the general open filter — the Browse
/// dialog narrows what Open already accepts rather than admitting anything new.
#[test]
fn the_template_filter_is_a_subset_of_the_open_filter() {
    for mime in TEMPLATE_MIME_TYPES {
        assert!(
            MIME_TYPES.contains(mime),
            "{mime} is offered by the template dialog but not by Open"
        );
    }
    assert!(
        TEMPLATE_MIME_TYPES.len() < MIME_TYPES.len(),
        "the template filter should be narrower than the open filter"
    );
}
