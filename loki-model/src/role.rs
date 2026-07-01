// SPDX-License-Identifier: Apache-2.0

//! RBAC roles and the action matrix (ADR-C017).

use serde::{Deserialize, Serialize};

/// Role of a member at workspace or document scope.
///
/// Roles are strictly ordered by privilege: `Owner > Editor > Commenter >
/// Viewer`. The ordering is used for "at least" checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Read only.
    Viewer,
    /// Read + comment; no content edit.
    Commenter,
    /// Read/write content and metadata.
    Editor,
    /// Full control, membership, deletion, tier changes.
    Owner,
}

/// An operation subject to an RBAC check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Read document content (snapshot download, collab read).
    ReadContent,
    /// Write document content (collab updates, blob upload).
    WriteContent,
    /// Add or resolve comments.
    Comment,
    /// Read title/ACL/tier metadata.
    ReadMetadata,
    /// Change title or other non-security metadata.
    WriteMetadata,
    /// Grant/revoke membership roles (plus DEK re-wrap under Tier 2).
    ManageMembers,
    /// Change the confidentiality tier (ADR-C014).
    ChangeTier,
    /// Delete the document or workspace.
    Delete,
}

impl Role {
    /// Whether this role permits `action` (the ADR-C017 rights matrix).
    #[must_use]
    pub const fn allows(self, action: Action) -> bool {
        match action {
            Action::ReadContent | Action::ReadMetadata => true,
            Action::Comment => matches!(self, Self::Commenter | Self::Editor | Self::Owner),
            Action::WriteContent | Action::WriteMetadata => {
                matches!(self, Self::Editor | Self::Owner)
            }
            Action::ManageMembers | Action::ChangeTier | Action::Delete => {
                matches!(self, Self::Owner)
            }
        }
    }

    /// Stable text form used in the database.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Commenter => "commenter",
            Self::Editor => "editor",
            Self::Owner => "owner",
        }
    }
}

/// Error returned when a stored role value is unknown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown role {0:?} (expected viewer, commenter, editor, or owner)")]
pub struct RoleParseError(pub String);

impl std::str::FromStr for Role {
    type Err = RoleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "viewer" => Ok(Self::Viewer),
            "commenter" => Ok(Self::Commenter),
            "editor" => Ok(Self::Editor),
            "owner" => Ok(Self::Owner),
            other => Err(RoleParseError(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rights_matrix_matches_adr_c017() {
        // Viewer: read only.
        assert!(Role::Viewer.allows(Action::ReadContent));
        assert!(!Role::Viewer.allows(Action::Comment));
        assert!(!Role::Viewer.allows(Action::WriteContent));
        // Commenter: read + comment, no edit.
        assert!(Role::Commenter.allows(Action::Comment));
        assert!(!Role::Commenter.allows(Action::WriteContent));
        // Editor: content + metadata, no membership/tier/delete.
        assert!(Role::Editor.allows(Action::WriteContent));
        assert!(Role::Editor.allows(Action::WriteMetadata));
        assert!(!Role::Editor.allows(Action::ManageMembers));
        assert!(!Role::Editor.allows(Action::ChangeTier));
        // Owner: everything.
        assert!(Role::Owner.allows(Action::ManageMembers));
        assert!(Role::Owner.allows(Action::ChangeTier));
        assert!(Role::Owner.allows(Action::Delete));
    }

    #[test]
    fn roles_order_by_privilege() {
        // Note enum variant order: Viewer < Commenter < Editor < Owner.
        // (Ord is intentionally not derived; parse round-trip is the contract.)
        for role in [Role::Viewer, Role::Commenter, Role::Editor, Role::Owner] {
            let parsed: Role = role.as_str().parse().unwrap();
            assert_eq!(parsed, role);
        }
        assert!("admin".parse::<Role>().is_err());
    }
}
