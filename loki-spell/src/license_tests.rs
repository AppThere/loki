// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::{Consent, LicenseClass};

#[test]
fn only_permissive_is_bundleable() {
    assert!(LicenseClass::Permissive.is_bundleable());
    assert!(!LicenseClass::LesserCopyleft.is_bundleable());
    assert!(!LicenseClass::Copyleft.is_bundleable());
}

#[test]
fn copyleft_requires_consent() {
    assert!(!LicenseClass::Permissive.requires_consent());
    assert!(LicenseClass::LesserCopyleft.requires_consent());
    assert!(LicenseClass::Copyleft.requires_consent());
}

#[test]
fn consent_gate() {
    // Permissive: satisfied regardless of consent.
    assert!(Consent::Denied.satisfies(LicenseClass::Permissive));
    assert!(Consent::Granted.satisfies(LicenseClass::Permissive));
    // Copyleft: only with explicit consent.
    assert!(!Consent::Denied.satisfies(LicenseClass::Copyleft));
    assert!(Consent::Granted.satisfies(LicenseClass::Copyleft));
    assert!(!Consent::Denied.satisfies(LicenseClass::LesserCopyleft));
    assert!(Consent::Granted.satisfies(LicenseClass::LesserCopyleft));
}
