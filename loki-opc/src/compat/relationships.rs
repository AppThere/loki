// SPDX-License-Identifier: MIT
// Copyright 2026 AppThere Loki contributors

//! Relationship-part compatibility notes ([MS-OI29500] / [MS-OE376]).

// Duplicate relationship ids are filtered during parsing: `RelationshipSet::add`
// keeps the first occurrence of each id, so no separate pass is needed here.
