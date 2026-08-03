// SPDX-License-Identifier: Apache-2.0

//! Applying the app-scoped document defaults to a **new blank document**
//! (Spec 08 T6.3, decision D-07).
//!
//! # Why this is a separate step rather than part of `Document::new_blank`
//!
//! `new_blank` lives in `loki_doc_model`, which knows nothing about a user's
//! machine — and should not: the same function builds the blank documents that
//! the headless converter and the test suites use, where reading a settings file
//! from the developer's home directory would make results depend on who ran
//! them. The defaults are an *application* concern, so they are applied at the
//! application's one blank-document seam.
//!
//! # Applied on the Blank arm alone
//!
//! `load_document` has four arms: blank, bundled template, imported file, and a
//! real file path. Only the first gets these. A template carries the geometry
//! its designer chose; an imported or opened file carries its author's. Seeding
//! those would silently reformat other people's documents to the reader's
//! preferences, which is the failure D-07's "seed, never embed" is written
//! against — and it would be near-invisible, because the result still looks like
//! a well-formed document.

use loki_app_shell::document_defaults::DocumentDefaults;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageSize;
use loki_doc_model::loki_primitives::units::Points;

/// Applies `defaults` to a freshly built blank `doc`, in place.
///
/// A field the user has not set is left alone rather than overwritten with a
/// built-in: `Document::new_blank` has already chosen a locale-appropriate page
/// size, and replacing it with a hardcoded one would make an unset preference
/// *worse* than no preference at all.
///
/// Pure over its inputs — no file is read here — so the seeding rule is testable
/// without a settings file on disk.
pub(super) fn apply_document_defaults(doc: &mut Document, defaults: &DocumentDefaults) {
    for section in &mut doc.sections {
        if let Some(size) = defaults.page_size {
            section.layout.page_size = PageSize {
                width: size.width,
                height: size.height,
            };
        }
        if let Some(m) = defaults.margins {
            // Only the four edges: header/footer/gutter distances are not part
            // of D-07's list, and clobbering them would discard geometry the
            // blank-document builder set deliberately.
            section.layout.margins.top = m.top;
            section.layout.margins.bottom = m.bottom;
            section.layout.margins.left = m.left;
            section.layout.margins.right = m.right;
        }
    }
}

/// The user's explicit measurement unit, for T6.4's resolution chain.
///
/// Reads the settings file; `None` when no choice has been recorded, which is
/// what makes the environment the next rung rather than this one.
#[must_use]
pub fn explicit_measurement_unit() -> Option<loki_doc_model::loki_primitives::units::MeasurementUnit>
{
    DocumentDefaults::load().measurement_unit
}

/// The sizes the user has entered by hand, most recent first, as page sizes.
#[must_use]
pub fn remembered_custom_sizes() -> Vec<PageSize> {
    DocumentDefaults::load()
        .custom_sizes
        .into_iter()
        .map(|s| PageSize {
            width: s.width,
            height: s.height,
        })
        .collect()
}

/// Records the current page geometry as the app-scoped default for **new**
/// documents — the "Use as default" action.
///
/// Takes the four margins and the page size; header/footer/gutter are not part
/// of D-07's list and are not recorded, which is the same boundary
/// [`apply_document_defaults`] honours when applying them.
pub fn set_default_page_geometry(layout: &loki_doc_model::layout::page::PageLayout) {
    let mut defaults = DocumentDefaults::load();
    defaults.page_size = Some(loki_app_shell::document_defaults::DefaultPageSize {
        width: Points::new(layout.page_size.width.value()),
        height: Points::new(layout.page_size.height.value()),
    });
    defaults.margins = Some(loki_app_shell::document_defaults::DefaultMargins {
        top: Points::new(layout.margins.top.value()),
        bottom: Points::new(layout.margins.bottom.value()),
        left: Points::new(layout.margins.left.value()),
        right: Points::new(layout.margins.right.value()),
    });
    defaults.save();
}

/// Clears the recorded page geometry, returning new documents to the
/// locale-derived answer.
///
/// "Use as default" without this would be a one-way door: a reader who set a
/// default by accident could only replace it, never get back the behaviour they
/// had before choosing.
pub fn clear_default_page_geometry() {
    let mut defaults = DocumentDefaults::load();
    defaults.page_size = None;
    defaults.margins = None;
    defaults.save();
}

/// Whether any page geometry has been recorded — drives whether the reset
/// control is worth showing.
#[must_use]
pub fn has_default_page_geometry() -> bool {
    let d = DocumentDefaults::load();
    d.page_size.is_some() || d.margins.is_some()
}

/// Records the measurement unit to display and type in (T6.4's explicit
/// setting), which until now had no writer.
pub fn set_measurement_unit(unit: loki_doc_model::loki_primitives::units::MeasurementUnit) {
    let mut defaults = DocumentDefaults::load();
    defaults.measurement_unit = Some(unit);
    defaults.save();
}

/// Records `size` as a hand-entered custom size, if the paper catalogue cannot
/// name it.
///
/// The "can the catalogue name it" question is answered **here** rather than in
/// `loki_app_shell`: the catalogue lives in `loki_doc_model`, above that layer,
/// and duplicating the lookup below it would be a second definition of what
/// counts as a custom size.
pub fn remember_custom_size(size: &PageSize) {
    let mut defaults = DocumentDefaults::load();
    let entry = loki_app_shell::document_defaults::DefaultPageSize {
        width: Points::new(size.width.value()),
        height: Points::new(size.height.value()),
    };
    let is_named = size.paper().is_some();
    let before = defaults.custom_sizes.clone();
    defaults.remember_custom_size(entry, is_named);
    if defaults.custom_sizes != before {
        defaults.save();
    }
}

#[cfg(test)]
#[path = "editor_defaults_tests.rs"]
mod tests;
