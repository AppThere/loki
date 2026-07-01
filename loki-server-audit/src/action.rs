// SPDX-License-Identifier: Apache-2.0

//! The audited action vocabulary (ADR-C020).

use serde::{Deserialize, Serialize};

/// What happened. The set is deliberately closed: security-relevant events
/// are enumerated so reviewers can see exactly what is (and is not) audited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditAction {
    /// A user authenticated (OIDC token accepted).
    AuthLogin,
    /// A user's session/token was rejected or revoked.
    AuthDenied,
    /// A workspace was created.
    WorkspaceCreate,
    /// A document was created.
    DocumentCreate,
    /// A membership role was granted, changed, or revoked.
    AclChange,
    /// A document or workspace confidentiality tier changed (ADR-C014).
    TierChange,
    /// A server-side export/print/convert was requested.
    Export,
    /// A document or workspace was deleted.
    Delete,
    /// A GDPR data-portability export was produced.
    GdprExport,
    /// A GDPR right-to-erasure request was executed (crypto-shredding).
    GdprErase,
}

impl AuditAction {
    /// Stable text form written into the audit table and the hash input.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthLogin => "auth-login",
            Self::AuthDenied => "auth-denied",
            Self::WorkspaceCreate => "workspace-create",
            Self::DocumentCreate => "document-create",
            Self::AclChange => "acl-change",
            Self::TierChange => "tier-change",
            Self::Export => "export",
            Self::Delete => "delete",
            Self::GdprExport => "gdpr-export",
            Self::GdprErase => "gdpr-erase",
        }
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
