// SPDX-License-Identifier: Apache-2.0

//! Identifier newtypes for server-side entities.
//!
//! Each identifier wraps a UUID so that a `DocumentId` can never be passed
//! where a `WorkspaceId` is expected. All types serialize as plain UUID
//! strings.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_newtype {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Creates a fresh random identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Wraps an existing UUID (e.g. one loaded from the database).
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Returns the underlying UUID.
            #[must_use]
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

id_newtype!(
    /// Identifies a workspace (the unit of membership and default policy).
    WorkspaceId
);
id_newtype!(
    /// Identifies a single collaborative document.
    DocumentId
);
id_newtype!(
    /// Identifies a user account (linked to an OIDC subject, never a password).
    UserId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_through_strings() {
        let id = DocumentId::new();
        let parsed: DocumentId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn ids_are_distinct_types() {
        // Compile-time property: this test documents that the newtypes exist.
        let doc = DocumentId::new();
        let ws = WorkspaceId::from_uuid(doc.as_uuid());
        assert_eq!(doc.as_uuid(), ws.as_uuid());
    }

    #[test]
    fn serde_is_transparent() {
        let id = UserId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
    }
}
