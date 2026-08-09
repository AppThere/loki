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

use dioxus::prelude::*;
use loki_app_shell::document_defaults::DocumentDefaults;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageSize;
use loki_doc_model::loki_primitives::units::{MeasurementUnit, Points, effective_measurement_unit};

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
            // Through `set_page_size`, so a landscape default does not seed
            // every new document with landscape dimensions under a `Portrait`
            // flag — which the DOCX writer would export as `w:orient="portrait"`.
            section.layout.set_page_size(PageSize {
                width: size.width,
                height: size.height,
            });
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

/// Everything the style panel needs from the app-scoped settings, read **once**
/// per render.
///
/// Each field used to have its own accessor, and each accessor opened and
/// parsed the settings file: the panel did four blocking reads per render, on
/// the UI thread, repeated on every caret move while it was open. One load,
/// threaded down, is the same information at a quarter of the syscalls — and
/// it also removes the chance of two halves of one render disagreeing because
/// the file changed between their reads.
#[derive(Clone, PartialEq)]
pub(super) struct PanelSettings {
    /// The unit to display and type in (T6.4's chain, already resolved).
    pub unit: MeasurementUnit,
    /// Hand-entered sizes, most recent first.
    pub custom_sizes: Vec<PageSize>,
    /// Whether any page geometry has been recorded — drives the Reset control.
    pub has_page_geometry: bool,
}

impl PanelSettings {
    /// Loads the settings and **subscribes the calling render scope** to the
    /// next write.
    ///
    /// The subscription is the point of taking `generation`. A settings file is
    /// not reactive state, so writing one changes nothing on screen by itself;
    /// the counter is what the panel re-renders on. It was passed to the two
    /// controls that *write* it and read by nothing during a render, so nobody
    /// subscribed and the bump was inert — picking a new unit left every number
    /// on screen in the old one until an unrelated event happened to redraw.
    ///
    /// Taking it here rather than reading it at some call site is rule 5: the
    /// only way to get the settings is to subscribe to changes in them.
    pub(super) fn load(generation: Signal<u64>) -> Self {
        // The read is the subscription — the value itself is not needed.
        let _generation = generation();
        let stored = DocumentDefaults::load();
        Self {
            unit: effective_measurement_unit(stored.measurement_unit),
            custom_sizes: stored
                .custom_sizes
                .iter()
                .map(|s| PageSize {
                    width: s.width,
                    height: s.height,
                })
                .collect(),
            has_page_geometry: stored.page_size.is_some() || stored.margins.is_some(),
        }
    }
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
