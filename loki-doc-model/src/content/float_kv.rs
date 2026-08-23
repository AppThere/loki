// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Keyword codecs for the float enums, split out of `float.rs` for the 300-line
//! ceiling.
//!
//! These are the `NodeAttr` `kv` spellings that carry a float's wrap
//! configuration through the Loro CRDT — one `as_kv`/`from_kv` pair per enum,
//! with no state and no dependency on `FloatWrap` itself. `FloatWrap`'s own
//! `store`/`read`, which assemble those keys into an attr, stay in `float.rs`.

use super::float::{FloatAlign, TextWrap, WrapSide};

impl TextWrap {
    pub(super) fn as_kv(self) -> &'static str {
        match self {
            TextWrap::Square => "square",
            TextWrap::Tight => "tight",
            TextWrap::Through => "through",
            TextWrap::TopAndBottom => "top-bottom",
            TextWrap::None => "none",
        }
    }

    pub(super) fn from_kv(s: &str) -> Option<Self> {
        Some(match s {
            "square" => TextWrap::Square,
            "tight" => TextWrap::Tight,
            "through" => TextWrap::Through,
            "top-bottom" => TextWrap::TopAndBottom,
            "none" => TextWrap::None,
            _ => return None,
        })
    }
}

impl WrapSide {
    pub(super) fn as_kv(self) -> &'static str {
        match self {
            WrapSide::Both => "both",
            WrapSide::Left => "left",
            WrapSide::Right => "right",
            WrapSide::Largest => "largest",
        }
    }

    pub(super) fn from_kv(s: &str) -> Self {
        match s {
            "left" => WrapSide::Left,
            "right" => WrapSide::Right,
            "largest" => WrapSide::Largest,
            _ => WrapSide::Both,
        }
    }
}

impl FloatAlign {
    pub(super) fn as_kv(self) -> &'static str {
        match self {
            FloatAlign::Left => "left",
            FloatAlign::Right => "right",
            FloatAlign::Center => "center",
        }
    }

    pub(super) fn from_kv(s: &str) -> Option<Self> {
        Self::from_str_kw(s)
    }

    /// Parses an OOXML `wp:positionH/wp:align` keyword.
    ///
    /// `inside`/`outside` are book-fold aliases that depend on page parity;
    /// they map to the recto reading (`inside` = left, `outside` = right) since
    /// mirrored margins are handled elsewhere. Anything unrecognised — notably a
    /// `wp:posOffset` number — is `None`, i.e. *unstated*, so the caller falls
    /// back to inferring from the wrap side rather than guessing a placement.
    #[must_use]
    pub fn from_str_kw(s: &str) -> Option<Self> {
        Some(match s {
            "left" | "inside" => FloatAlign::Left,
            "right" | "outside" => FloatAlign::Right,
            "center" | "centre" => FloatAlign::Center,
            _ => return None,
        })
    }
}
