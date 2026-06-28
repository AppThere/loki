// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Dictionary license classification and bundling/consent policy.
//!
//! Hunspell dictionaries ship under a range of licenses. This module reduces an
//! SPDX expression (carried as data in the catalog) to a broad [`LicenseClass`]
//! and encodes the project policy:
//!
//! - Only **permissive** dictionaries may be *bundled* in the application.
//! - **Copyleft** and **lesser-copyleft** dictionaries may still be *downloaded*
//!   on demand, but only after explicit, recorded user consent — preserving the
//!   user's right to obtain, modify, and redistribute them under their terms.

use serde::{Deserialize, Serialize};

/// Broad license classification driving bundling and consent decisions.
///
/// The precise SPDX expression is preserved alongside this (see
/// [`crate::catalog::DictionaryEntry::license_spdx`]); the class is the coarse
/// bucket the policy acts on. For an `OR` expression the class reflects the
/// *least restrictive* option the user may elect (e.g.
/// `GPL-3.0 OR LGPL-3.0 OR MPL-1.1` is [`LicenseClass::LesserCopyleft`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LicenseClass {
    /// MIT, BSD, Apache, ISC, Unlicense, public domain, etc. — no copyleft
    /// obligation. Bundleable and freely redistributable.
    Permissive,
    /// Weak / file-level or lesser copyleft (LGPL, MPL): redistributable, but
    /// changes to the licensed files stay under the same terms.
    LesserCopyleft,
    /// Strong copyleft (GPL): redistributable, but imposes obligations on the
    /// combined work — never bundled, downloaded only on explicit consent.
    Copyleft,
}

impl LicenseClass {
    /// Whether a dictionary of this class may be **bundled** in the app binary.
    ///
    /// Only permissive dictionaries qualify.
    pub fn is_bundleable(self) -> bool {
        matches!(self, LicenseClass::Permissive)
    }

    /// Whether **installing** a dictionary of this class requires explicit,
    /// recorded user consent (true for anything with copyleft obligations).
    pub fn requires_consent(self) -> bool {
        !matches!(self, LicenseClass::Permissive)
    }
}

/// A caller's decision about installing a dictionary that requires consent.
///
/// Permissive dictionaries ignore this; copyleft / lesser-copyleft installs are
/// refused with [`crate::SpellError::ConsentRequired`] unless
/// [`Consent::Granted`] is passed, forcing the UI to obtain a deliberate choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consent {
    /// The user has explicitly accepted this dictionary's license terms.
    Granted,
    /// No consent given; a consent-requiring install will be refused.
    Denied,
}

impl Consent {
    /// Returns `true` if the policy is satisfied for `class`.
    pub(crate) fn satisfies(self, class: LicenseClass) -> bool {
        !class.requires_consent() || matches!(self, Consent::Granted)
    }
}

#[cfg(test)]
#[path = "license_tests.rs"]
mod tests;
