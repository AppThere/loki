// SPDX-License-Identifier: Apache-2.0

//! RBAC enforcement helper (ADR-C017).

use loki_model::{Action, Role};

/// Why access was denied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AccessError {
    /// The caller has no membership on the target at all.
    #[error("caller is not a member of the target")]
    NotMember,
    /// The caller's role does not permit the action.
    #[error("role {role:?} does not permit {action:?}")]
    Forbidden {
        /// The caller's role.
        role: Role,
        /// The denied action.
        action: Action,
    },
}

/// Checks that `role` (the caller's membership, if any) permits `action`.
///
/// Returns the role on success so handlers can make further role-dependent
/// decisions without a second lookup.
pub fn require(role: Option<Role>, action: Action) -> Result<Role, AccessError> {
    let role = role.ok_or(AccessError::NotMember)?;
    if role.allows(action) {
        Ok(role)
    } else {
        Err(AccessError::Forbidden { role, action })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_member_is_distinct_from_forbidden() {
        assert_eq!(require(None, Action::ReadContent), Err(AccessError::NotMember));
        assert_eq!(
            require(Some(Role::Viewer), Action::WriteContent),
            Err(AccessError::Forbidden {
                role: Role::Viewer,
                action: Action::WriteContent
            })
        );
        assert_eq!(require(Some(Role::Editor), Action::WriteContent), Ok(Role::Editor));
    }
}
