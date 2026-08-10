// SPDX-License-Identifier: Apache-2.0

//! The standing preflight (design notes 27, 28 and 30).
//!
//! # Warnings never block
//!
//! Every check here is computed from the live document, and none of them stops
//! a publish. The dialog names what is wrong, links to where it lives, and lets
//! the user publish anyway once they have seen the cost — the distinction the
//! design draws between a *warning* and an *error*, and the reason the primary
//! button stays live and honest.

use loki_doc_model::Document;
use loki_i18n::fl;

use super::audit;

/// How serious a preflight finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Severity {
    /// A check that passed.
    Pass,
    /// Something a store or a reader may object to. Never blocks.
    Warning,
    /// Something that would make the package invalid. Blocks the publish.
    Error,
}

/// Where a finding can be fixed, so the dialog can offer the jump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FixIn {
    /// The metadata dialog.
    Metadata,
    /// Somewhere in the document body.
    Document,
    /// Nowhere — the finding is informational.
    None,
}

/// One preflight row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Finding {
    /// How serious it is.
    pub severity: Severity,
    /// The already-localized message.
    pub message: String,
    /// Which dialog fixes it.
    pub fix_in: FixIn,
}

/// The whole preflight report.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Preflight {
    /// Every finding, most serious first.
    pub findings: Vec<Finding>,
}

impl Preflight {
    /// How many checks passed.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.count(Severity::Pass)
    }

    /// How many warnings there are.
    #[must_use]
    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    /// How many errors there are.
    #[must_use]
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Whether the publish may proceed.
    ///
    /// **Warnings do not block** (note 30). Only an error does — a package that
    /// would fail to open is not a matter of taste.
    #[must_use]
    pub fn can_publish(&self) -> bool {
        self.errors() == 0
    }

    /// The footer's summary line.
    #[must_use]
    pub fn summary(&self) -> String {
        fl!(
            "publish-dialog-preflight-summary",
            passed = self.passed() as i64,
            warnings = self.warnings() as i64,
            errors = self.errors() as i64
        )
    }
}

/// Runs every check against `doc`.
#[must_use]
pub(super) fn run(doc: &Document) -> Preflight {
    let meta = &doc.meta;
    let dc = &meta.dublin_core;
    let mut findings = Vec::new();

    // EPUB 3.3 §5.4 requires title, language and a unique identifier in the
    // package document. These are the checks that are genuinely errors.
    push(
        &mut findings,
        meta.title.as_deref().is_some_and(|s| !s.trim().is_empty()),
        Severity::Error,
        fl!("publish-dialog-check-title-ok"),
        fl!("publish-dialog-check-title-missing"),
        FixIn::Metadata,
    );
    push(
        &mut findings,
        meta.language.is_some(),
        Severity::Error,
        fl!("publish-dialog-check-language-ok"),
        fl!("publish-dialog-check-language-missing"),
        FixIn::Metadata,
    );
    // The identifier is generated on first save, so a document that has never
    // been saved legitimately has none yet — a warning, not an error.
    push(
        &mut findings,
        dc.identifier
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty()),
        Severity::Warning,
        fl!("publish-dialog-check-identifier-ok"),
        fl!("publish-dialog-check-identifier-missing"),
        FixIn::Metadata,
    );
    push(
        &mut findings,
        meta.creator
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty()),
        Severity::Warning,
        fl!("publish-dialog-check-creator-ok"),
        fl!("publish-dialog-check-creator-missing"),
        FixIn::Metadata,
    );
    // Not required by the specification, but by every store that ingests one —
    // the distinction the design draws rather than eliding.
    push(
        &mut findings,
        dc.publisher
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty()),
        Severity::Warning,
        fl!("publish-dialog-check-publisher-ok"),
        fl!("publish-dialog-check-publisher-missing"),
        FixIn::Metadata,
    );

    let structure = audit::heading_structure(doc);
    push(
        &mut findings,
        structure.headings > 0,
        Severity::Warning,
        fl!(
            "publish-dialog-check-headings-ok",
            count = structure.headings as i64
        ),
        fl!("publish-dialog-check-headings-missing"),
        FixIn::Document,
    );
    push(
        &mut findings,
        structure.well_nested,
        Severity::Warning,
        fl!("publish-dialog-check-nesting-ok"),
        fl!("publish-dialog-check-nesting-skipped"),
        FixIn::Document,
    );

    let tables = audit::table_audit(doc);
    if tables.total > 0 {
        push(
            &mut findings,
            tables.total == tables.accessible,
            Severity::Warning,
            fl!(
                "publish-dialog-check-tables-ok",
                count = tables.total as i64
            ),
            fl!(
                "publish-dialog-check-tables-bare",
                count = (tables.total - tables.accessible) as i64
            ),
            FixIn::Document,
        );
    }

    let images = audit::image_audit(doc);
    if images.total > 0 {
        push(
            &mut findings,
            images.total == images.described,
            Severity::Warning,
            fl!(
                "publish-dialog-check-images-ok",
                count = images.total as i64
            ),
            fl!(
                "publish-dialog-check-images-bare",
                count = (images.total - images.described) as i64
            ),
            FixIn::Document,
        );
    }

    // Most serious first, so the rail's top row is the one worth reading.
    findings.sort_by_key(|f| std::cmp::Reverse(f.severity));
    Preflight { findings }
}

/// Appends the pass or the failure, whichever applies.
fn push(
    findings: &mut Vec<Finding>,
    ok: bool,
    severity: Severity,
    pass_message: String,
    fail_message: String,
    fix_in: FixIn,
) {
    findings.push(if ok {
        Finding {
            severity: Severity::Pass,
            message: pass_message,
            fix_in: FixIn::None,
        }
    } else {
        Finding {
            severity,
            message: fail_message,
            fix_in,
        }
    });
}

#[cfg(test)]
#[path = "preflight_tests.rs"]
mod tests;
