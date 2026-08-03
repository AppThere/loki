// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! App-scoped document defaults — page size, margins, measurement unit, and
//! remembered custom sizes (Spec 08 T6.3, decision D-07).
//!
//! # Seed, never embed
//!
//! These are the settings a **new blank document** starts from. They are applied
//! once, at creation, as ordinary geometry: the document that results is
//! indistinguishable from one the user set up by hand, and carries no reference
//! back to this file. Two consequences follow, and both are load-bearing:
//!
//! * **Opening an existing document must not consult them.** A file's geometry
//!   is the author's, not the reader's; re-seeding on open would silently
//!   reformat every document that arrived from someone whose defaults differ.
//!   `loki_text`'s `load_document` applies them on its `Blank` arm alone —
//!   templates and imports carry their own.
//! * **Nothing here is written into any document format.** There is no "app
//!   default" marker to export, because the value was resolved before the
//!   document existed.
//!
//! # Styles stay document-scoped
//!
//! D-07 draws the line at geometry. This struct holds lengths and a unit; it
//! holds no style definitions, and `no_style_state_is_persisted` asserts the
//! serialised form stays that way. A style catalogue that lived here would make
//! a document render differently on two machines, which is the failure this
//! whole "seed, never embed" rule exists to prevent.
//!
//! # A size is stored as its dimensions
//!
//! Not as a paper name. The catalogue derives names *from* dimensions
//! (`loki_doc_model::layout::paper_catalog`), so storing a name would be a
//! second spelling of the same fact, and a name this build did not recognise
//! would have no dimensions to fall back on.

use std::path::PathBuf;

use loki_primitives::units::{MeasurementUnit, Points};
use serde::{Deserialize, Serialize};

/// Relative path under the platform data directory.
///
/// Suite-shared, like the display calibration: a reader who set A4 in Text meant
/// "my paper is A4", not "my paper is A4 in this one application".
pub const DEFAULTS_FILE: &str = "AppThere/document-defaults.json";

/// The widest and narrowest page edge that will be accepted from the settings
/// file, in points. A file edited by hand — or written by a future version — is
/// untrusted input like any other; an implausible page is dropped in favour of
/// the built-in default rather than propagated into every new document.
const MIN_EDGE_PT: f64 = 36.0;
const MAX_EDGE_PT: f64 = 14400.0;

/// The most custom sizes remembered. Oldest are dropped first; the list is a
/// convenience, and an unbounded one would grow with every experiment.
pub const MAX_CUSTOM_SIZES: usize = 8;

/// A page size as two lengths — see the module docs on why not a name.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct DefaultPageSize {
    /// Page width.
    pub width: Points,
    /// Page height.
    pub height: Points,
}

impl DefaultPageSize {
    /// Whether both edges are finite and within the plausible range.
    #[must_use]
    pub fn is_plausible(&self) -> bool {
        let ok = |v: f64| v.is_finite() && (MIN_EDGE_PT..=MAX_EDGE_PT).contains(&v);
        ok(self.width.value()) && ok(self.height.value())
    }
}

/// Page margins as four edges.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct DefaultMargins {
    /// Top margin.
    pub top: Points,
    /// Bottom margin.
    pub bottom: Points,
    /// Left (or inside) margin.
    pub left: Points,
    /// Right (or outside) margin.
    pub right: Points,
}

impl DefaultMargins {
    /// Whether every edge is finite and non-negative. Margins are checked
    /// separately from the page because a zero margin is legitimate (full-bleed)
    /// while a zero-width page is not.
    #[must_use]
    pub fn is_plausible(&self) -> bool {
        let ok = |v: f64| v.is_finite() && (0.0..=MAX_EDGE_PT).contains(&v);
        ok(self.top.value())
            && ok(self.bottom.value())
            && ok(self.left.value())
            && ok(self.right.value())
    }
}

/// The app-scoped defaults. Every field is optional: absent means "no choice
/// recorded", which is **not** the same as a recorded value that happens to
/// match the built-in — the caller falls back to the locale-derived answer only
/// in the first case.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DocumentDefaults {
    /// Page size for new documents.
    pub page_size: Option<DefaultPageSize>,
    /// Margins for new documents.
    pub margins: Option<DefaultMargins>,
    /// The unit lengths are shown and typed in (T6.4's explicit setting).
    pub measurement_unit: Option<MeasurementUnit>,
    /// Sizes the user entered by hand, most recent first, so the picker can
    /// offer them again.
    pub custom_sizes: Vec<DefaultPageSize>,
}

impl DocumentDefaults {
    /// Loads the defaults, or the empty set when the file is missing or
    /// unreadable.
    ///
    /// **Implausible values are dropped on load rather than at the point of
    /// use.** A caller that reads this struct is entitled to assume its contents
    /// are usable; validating here means there is one place that decides, and no
    /// path by which a hand-edited zero-height page reaches a new document.
    #[must_use]
    pub fn load() -> Self {
        let mut me = Self::load_raw();
        if me.page_size.is_some_and(|s| !s.is_plausible()) {
            tracing::warn!("document defaults: implausible page size ignored");
            me.page_size = None;
        }
        if me.margins.is_some_and(|m| !m.is_plausible()) {
            tracing::warn!("document defaults: implausible margins ignored");
            me.margins = None;
        }
        me.custom_sizes.retain(DefaultPageSize::is_plausible);
        me.custom_sizes.truncate(MAX_CUSTOM_SIZES);
        me
    }

    /// The stored contents, unvalidated.
    fn load_raw() -> Self {
        let Some(path) = defaults_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default(); // absent is the ordinary first-run state
        };
        match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(?err, ?path, "could not parse the document defaults");
                Self::default()
            }
        }
    }

    /// Records `size` as a size the user entered by hand, most recent first.
    ///
    /// `is_named` says whether the paper catalogue can name this size; a named
    /// one is **not** recorded, because the list exists to bring back sizes the
    /// catalogue cannot offer. The caller passes the answer rather than this
    /// crate computing it — the catalogue lives in `loki_doc_model`, above this
    /// layer.
    pub fn remember_custom_size(&mut self, size: DefaultPageSize, is_named: bool) {
        if is_named || !size.is_plausible() {
            return;
        }
        self.custom_sizes.retain(|s| *s != size);
        self.custom_sizes.insert(0, size);
        self.custom_sizes.truncate(MAX_CUSTOM_SIZES);
    }

    /// Persist.
    ///
    /// A failure is reported, not discarded: silently losing the setting looks
    /// to the reader like the preference is broken rather than like the disk is
    /// full — the same reasoning as the display calibration's save.
    pub fn save(&self) {
        let Some(path) = defaults_path() else {
            tracing::warn!("no data directory: document defaults cannot be saved");
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(?err, ?parent, "could not create the settings directory");
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(err) = std::fs::write(&path, json) {
                    tracing::warn!(?err, ?path, "could not save the document defaults");
                }
            }
            Err(err) => tracing::warn!(?err, "could not serialise the document defaults"),
        }
    }
}

/// Where the defaults file lives on this platform.
///
/// Two `cfg`-selected functions rather than one with an early `return`, matching
/// [`crate::display_calibration`]'s reasoning.
#[cfg(target_os = "android")]
fn defaults_path() -> Option<PathBuf> {
    crate::recent_documents::android_data_dir().map(|d| d.join(DEFAULTS_FILE))
}

#[cfg(not(target_os = "android"))]
fn defaults_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(DEFAULTS_FILE))
}

#[cfg(test)]
#[path = "document_defaults_tests.rs"]
mod tests;
