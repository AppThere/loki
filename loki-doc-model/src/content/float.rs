// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Floating-object text-wrap mode for anchored images and shapes.
//!
//! A floating (anchored) drawing is positioned outside the inline text flow,
//! and surrounding text wraps around it according to a *wrap mode*. OOXML
//! `wp:anchor` children (`wp:wrapSquare`/`wp:wrapTight`/`wp:wrapThrough`/
//! `wp:wrapTopAndBottom`/`wp:wrapNone`); ODF `style:wrap` on the frame's
//! graphic style.
//!
//! Floating drawings are marked with the [`FLOATING_CLASS`] class on their
//! [`NodeAttr`]; the wrap detail is carried in `NodeAttr.kv` under reserved
//! keys (see [`FloatWrap::store`]/[`FloatWrap::read`]). The layout engine flows
//! text around side-wrapping floats (`loki-layout`'s `flow_float`); an image
//! tagged only with [`FLOATING_CLASS`] (no wrap keys) is still treated as
//! floating via [`FloatWrap::read_or_class_default`].

use crate::content::attr::NodeAttr;

/// Class marking an image/shape as floating (anchored) rather than inline.
pub const FLOATING_CLASS: &str = "floating";

const KV_WRAP: &str = "float-wrap";
const KV_SIDE: &str = "float-wrap-side";
const KV_BEHIND: &str = "float-behind";

/// How body text wraps around a floating object.
///
/// Neutral over OOXML wrap elements and ODF `style:wrap` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TextWrap {
    /// Text wraps around the object's bounding box.
    /// OOXML `wp:wrapSquare`; ODF `style:wrap="parallel"`.
    #[default]
    Square,
    /// Text wraps to the object's tight contour (wrap polygon).
    /// OOXML `wp:wrapTight`; ODF `style:wrap="parallel"` with a contour.
    Tight,
    /// Text flows through the object's transparent regions.
    /// OOXML `wp:wrapThrough`; ODF `style:wrap="run-through"` (parallel form).
    Through,
    /// Text stops above and resumes below; no text beside the object.
    /// OOXML `wp:wrapTopAndBottom`; ODF `style:wrap="none"`.
    TopAndBottom,
    /// No wrap: the object floats over or under the text.
    /// OOXML `wp:wrapNone`; ODF `style:wrap="run-through"` (run-through form).
    None,
}

/// Which side(s) of the object text is allowed to occupy.
///
/// OOXML `@wrapText` on the wrap element; ODF `style:wrap="left"/"right"/
/// "dynamic"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum WrapSide {
    /// Text on both sides. OOXML `bothSides`; ODF `parallel`.
    #[default]
    Both,
    /// Text on the left only. OOXML `left`; ODF `left`.
    Left,
    /// Text on the right only. OOXML `right`; ODF `right`.
    Right,
    /// Text on the larger side only. OOXML `largest`; ODF `dynamic`.
    Largest,
}

/// Text-wrap configuration for a floating object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FloatWrap {
    /// The wrap mode.
    pub wrap: TextWrap,
    /// Which side(s) text may occupy (meaningful for `Square`/`Tight`/`Through`).
    pub side: WrapSide,
    /// `true` when the object sits behind the text (OOXML `wp:wrapNone` with
    /// `behindDoc="1"`; ODF `style:run-through="background"`).
    pub behind_text: bool,
}

impl TextWrap {
    fn as_kv(self) -> &'static str {
        match self {
            TextWrap::Square => "square",
            TextWrap::Tight => "tight",
            TextWrap::Through => "through",
            TextWrap::TopAndBottom => "top-bottom",
            TextWrap::None => "none",
        }
    }

    fn from_kv(s: &str) -> Option<Self> {
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
    fn as_kv(self) -> &'static str {
        match self {
            WrapSide::Both => "both",
            WrapSide::Left => "left",
            WrapSide::Right => "right",
            WrapSide::Largest => "largest",
        }
    }

    fn from_kv(s: &str) -> Self {
        match s {
            "left" => WrapSide::Left,
            "right" => WrapSide::Right,
            "largest" => WrapSide::Largest,
            _ => WrapSide::Both,
        }
    }
}

impl FloatWrap {
    /// Writes this wrap configuration into `attr`: ensures the [`FLOATING_CLASS`]
    /// class is present and records the wrap mode/side/behind flag in `kv`.
    /// Idempotent — any previously stored wrap keys are replaced.
    pub fn store(&self, attr: &mut NodeAttr) {
        if !attr.classes.iter().any(|c| c == FLOATING_CLASS) {
            attr.classes.push(FLOATING_CLASS.to_string());
        }
        attr.kv
            .retain(|(k, _)| k != KV_WRAP && k != KV_SIDE && k != KV_BEHIND);
        attr.kv
            .push((KV_WRAP.to_string(), self.wrap.as_kv().to_string()));
        attr.kv
            .push((KV_SIDE.to_string(), self.side.as_kv().to_string()));
        if self.behind_text {
            attr.kv.push((KV_BEHIND.to_string(), "true".to_string()));
        }
    }

    /// Like [`read`](Self::read), but falls back to a default wrap when the attr
    /// is marked with [`FLOATING_CLASS`] yet carries **no** explicit wrap keys.
    ///
    /// A producer may tag an image as floating (anchored) without recording a
    /// wrap mode — e.g. an OOXML `wp:anchor` whose wrap child was absent or
    /// unrecognised (the DOCX mapper still adds [`FLOATING_CLASS`]). Such an
    /// image is floating, not inline, so this returns a square wrap on both
    /// sides (text flows around it) rather than `None`. Returns `None` for a
    /// genuinely inline attr (no wrap keys and no floating class).
    #[must_use]
    pub fn read_or_class_default(attr: &NodeAttr) -> Option<Self> {
        Self::read(attr).or_else(|| {
            attr.classes
                .iter()
                .any(|c| c == FLOATING_CLASS)
                .then(Self::class_default)
        })
    }

    /// The wrap used for an image marked [`FLOATING_CLASS`] with no explicit
    /// wrap keys: a square wrap on both sides, in front of the text.
    #[must_use]
    fn class_default() -> Self {
        FloatWrap {
            wrap: TextWrap::Square,
            side: WrapSide::Both,
            behind_text: false,
        }
    }

    /// Reads a wrap configuration previously stored on `attr`, if any.
    /// Returns `None` when no wrap mode is recorded.
    #[must_use]
    pub fn read(attr: &NodeAttr) -> Option<Self> {
        let wrap = attr
            .kv
            .iter()
            .find(|(k, _)| k == KV_WRAP)
            .and_then(|(_, v)| TextWrap::from_kv(v))?;
        let side = attr
            .kv
            .iter()
            .find(|(k, _)| k == KV_SIDE)
            .map(|(_, v)| WrapSide::from_kv(v))
            .unwrap_or_default();
        let behind_text = attr.kv.iter().any(|(k, v)| k == KV_BEHIND && v == "true");
        Some(FloatWrap {
            wrap,
            side,
            behind_text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_then_read_round_trips() {
        let mut attr = NodeAttr::default();
        let fw = FloatWrap {
            wrap: TextWrap::Tight,
            side: WrapSide::Left,
            behind_text: false,
        };
        fw.store(&mut attr);
        assert!(attr.classes.iter().any(|c| c == FLOATING_CLASS));
        assert_eq!(FloatWrap::read(&attr), Some(fw));
    }

    #[test]
    fn class_only_attr_reads_as_default_float() {
        // An anchored image tagged floating but with no wrap keys (e.g. a DOCX
        // `wp:anchor` with no wrap child) is floating, not inline.
        let mut attr = NodeAttr::default();
        attr.classes.push(FLOATING_CLASS.to_string());
        assert_eq!(FloatWrap::read(&attr), None, "no wrap keys → read is None");
        let fw = FloatWrap::read_or_class_default(&attr).expect("class marks it floating");
        assert_eq!(fw.wrap, TextWrap::Square);
        assert_eq!(fw.side, WrapSide::Both);
        assert!(!fw.behind_text);
    }

    #[test]
    fn inline_attr_reads_or_class_default_is_none() {
        // No wrap keys and no floating class → genuinely inline.
        let attr = NodeAttr::default();
        assert_eq!(FloatWrap::read_or_class_default(&attr), None);
    }

    #[test]
    fn explicit_wrap_wins_over_class_default() {
        // When wrap keys are present, the stored config is returned verbatim.
        let mut attr = NodeAttr::default();
        let fw = FloatWrap {
            wrap: TextWrap::Tight,
            side: WrapSide::Left,
            behind_text: false,
        };
        fw.store(&mut attr);
        assert_eq!(FloatWrap::read_or_class_default(&attr), Some(fw));
    }

    #[test]
    fn behind_text_round_trips() {
        let mut attr = NodeAttr::default();
        let fw = FloatWrap {
            wrap: TextWrap::None,
            side: WrapSide::Both,
            behind_text: true,
        };
        fw.store(&mut attr);
        assert_eq!(FloatWrap::read(&attr), Some(fw));
    }

    #[test]
    fn store_is_idempotent() {
        let mut attr = NodeAttr::default();
        FloatWrap {
            wrap: TextWrap::Square,
            side: WrapSide::Both,
            behind_text: false,
        }
        .store(&mut attr);
        FloatWrap {
            wrap: TextWrap::Through,
            side: WrapSide::Right,
            behind_text: false,
        }
        .store(&mut attr);
        // Only one floating class, one set of wrap keys.
        assert_eq!(
            attr.classes.iter().filter(|c| *c == FLOATING_CLASS).count(),
            1
        );
        assert_eq!(attr.kv.iter().filter(|(k, _)| k == KV_WRAP).count(), 1);
        assert_eq!(
            FloatWrap::read(&attr),
            Some(FloatWrap {
                wrap: TextWrap::Through,
                side: WrapSide::Right,
                behind_text: false,
            })
        );
    }

    #[test]
    fn read_none_when_absent() {
        assert_eq!(FloatWrap::read(&NodeAttr::default()), None);
    }
}
