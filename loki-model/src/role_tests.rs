// SPDX-License-Identifier: Apache-2.0

//! Exhaustive tests for the ADR-C017 rights matrix (`Role::allows`).
//!
//! The whole server's authorization ground truth is one `match` in
//! `role.rs`, so every one of the `Role` x `Action` cells is asserted here
//! against a **hand-written literal table** transcribed from ADR-C017. The
//! table is deliberately *not* computed from `Role::allows`: a table derived
//! from the function under test is the same fact twice and asserts nothing.

use super::*;

const ROLE_COUNT: usize = 4;
const ACTION_COUNT: usize = 8;

/// Every `Role`, in the row order of [`RIGHTS_MATRIX`].
const ALL_ROLES: [Role; ROLE_COUNT] = [Role::Viewer, Role::Commenter, Role::Editor, Role::Owner];

/// Every `Action`, in the column order of [`RIGHTS_MATRIX`].
const ALL_ACTIONS: [Action; ACTION_COUNT] = [
    Action::ReadContent,
    Action::WriteContent,
    Action::Comment,
    Action::ReadMetadata,
    Action::WriteMetadata,
    Action::ManageMembers,
    Action::ChangeTier,
    Action::Delete,
];

/// Position of `role` in [`ALL_ROLES`].
///
/// The exhaustive `match` is the coverage gate: adding a `Role` variant makes
/// this fail to compile, so a new role cannot slip into the enum without
/// getting a row in [`RIGHTS_MATRIX`].
const fn role_index(role: Role) -> usize {
    match role {
        Role::Viewer => 0,
        Role::Commenter => 1,
        Role::Editor => 2,
        Role::Owner => 3,
    }
}

/// Position of `action` in [`ALL_ACTIONS`] — same compile-time gate as
/// [`role_index`], for the matrix's columns.
const fn action_index(action: Action) -> usize {
    match action {
        Action::ReadContent => 0,
        Action::WriteContent => 1,
        Action::Comment => 2,
        Action::ReadMetadata => 3,
        Action::WriteMetadata => 4,
        Action::ManageMembers => 5,
        Action::ChangeTier => 6,
        Action::Delete => 7,
    }
}

/// The ADR-C017 rights matrix, transcribed by hand. Columns are
/// [`ALL_ACTIONS`] in order:
///
/// | | Read | Write | Cmnt | RdMeta | WrMeta | Members | Tier | Delete |
/// |---|---|---|---|---|---|---|---|---|
/// | Viewer    | y | . | . | y | . | . | . | . |
/// | Commenter | y | . | y | y | . | . | . | . |
/// | Editor    | y | y | y | y | y | . | . | . |
/// | Owner     | y | y | y | y | y | y  | y | y |
const RIGHTS_MATRIX: [(Role, [bool; ACTION_COUNT]); ROLE_COUNT] = [
    (
        Role::Viewer,
        [true, false, false, true, false, false, false, false],
    ),
    (
        Role::Commenter,
        [true, false, true, true, false, false, false, false],
    ),
    (
        Role::Editor,
        [true, true, true, true, true, false, false, false],
    ),
    (Role::Owner, [true; ACTION_COUNT]),
];

/// The `ALL_*` arrays must enumerate every variant exactly once, or the
/// matrix loop below would silently skip cells.
#[test]
fn all_roles_and_actions_are_enumerated_exactly_once() {
    for (i, role) in ALL_ROLES.iter().enumerate() {
        assert_eq!(role_index(*role), i, "ALL_ROLES[{i}] is out of order");
    }
    for (i, action) in ALL_ACTIONS.iter().enumerate() {
        assert_eq!(action_index(*action), i, "ALL_ACTIONS[{i}] is out of order");
    }
    // Rows must line up with ALL_ROLES so a reordered table cannot shift a
    // whole row of expectations onto the wrong role.
    for (i, (role, _)) in RIGHTS_MATRIX.iter().enumerate() {
        assert_eq!(*role, ALL_ROLES[i], "RIGHTS_MATRIX row {i} is out of order");
    }
}

/// Every one of the 32 role x action cells, positive and negative.
///
/// This is the test that must fail for mutations such as moving
/// `Action::Delete` out of the Owner-only arm into the
/// `WriteContent | WriteMetadata` arm (which would let Editors delete
/// documents).
#[test]
fn rights_matrix_matches_adr_c017_exhaustively() {
    let mut asserted = 0usize;
    for (role, expected_row) in RIGHTS_MATRIX {
        for action in ALL_ACTIONS {
            let expected = expected_row[action_index(action)];
            assert_eq!(
                role.allows(action),
                expected,
                "{role:?} x {action:?}: expected allows == {expected}"
            );
            asserted += 1;
        }
    }
    assert_eq!(
        asserted,
        ROLE_COUNT * ACTION_COUNT,
        "every cell is asserted"
    );
}

/// A counting gate needs a floor as well as a ceiling: pin how many cells are
/// permitted and how many are denied, so a table edited into all-`true` (or a
/// row silently duplicated) fails instead of passing vacuously.
#[test]
fn rights_matrix_is_two_sided() {
    let permitted = RIGHTS_MATRIX
        .iter()
        .flat_map(|(_, row)| row.iter())
        .filter(|allowed| **allowed)
        .count();
    let denied = ROLE_COUNT * ACTION_COUNT - permitted;
    assert_eq!(permitted, 18, "permitted cells in ADR-C017");
    assert_eq!(denied, 14, "denied cells in ADR-C017");
}

/// The `Role` doc comment claims a strict privilege order
/// (`Owner > Editor > Commenter > Viewer`). `Ord` is deliberately not
/// derived, so the claim is only meaningful as a property of the rights
/// matrix: each role must permit a **strict superset** of the next one down.
///
/// (Renamed from `roles_order_by_privilege`, which asserted parse round-trips
/// and no ordering at all — audit finding F-MO-3. The round-trip assertions
/// live in `role_str_round_trips_and_rejects_unknown` below.)
#[test]
fn privilege_order_is_a_strict_permission_chain() {
    for pair in ALL_ROLES.windows(2) {
        let (lower, higher) = (pair[0], pair[1]);
        let mut strictly_more = false;
        for action in ALL_ACTIONS {
            if lower.allows(action) {
                assert!(
                    higher.allows(action),
                    "{higher:?} outranks {lower:?} but is denied {action:?}"
                );
            } else if higher.allows(action) {
                strictly_more = true;
            }
        }
        assert!(
            strictly_more,
            "{higher:?} must permit strictly more than {lower:?}"
        );
    }
}

/// `as_str` is the stored database representation, so both the exact strings
/// and the parse round-trip are contracts.
#[test]
fn role_str_round_trips_and_rejects_unknown() {
    assert_eq!(Role::Viewer.as_str(), "viewer");
    assert_eq!(Role::Commenter.as_str(), "commenter");
    assert_eq!(Role::Editor.as_str(), "editor");
    assert_eq!(Role::Owner.as_str(), "owner");

    for role in ALL_ROLES {
        let parsed: Role = role.as_str().parse().unwrap();
        assert_eq!(parsed, role);
    }
    assert!("admin".parse::<Role>().is_err());
    assert!(
        "Owner".parse::<Role>().is_err(),
        "parsing is case-sensitive"
    );
    assert!("".parse::<Role>().is_err());
}
