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
const KV_ALIGN: &str = "float-align";
const KV_DIST_T: &str = "float-dist-t";
const KV_DIST_B: &str = "float-dist-b";
const KV_DIST_L: &str = "float-dist-l";
const KV_DIST_R: &str = "float-dist-r";

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

/// Where a floating object sits horizontally in its anchor's column.
///
/// OOXML `wp:positionH/wp:align`; ODF `style:horizontal-pos`.
///
/// Distinct from [`WrapSide`], which says which sides *text* may occupy. The
/// two agree for `left`/`right` wrap — text on the left implies the object is
/// on the right — but `bothSides` and `largest` constrain nothing, and then
/// only the position says where the object goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum FloatAlign {
    /// Against the column's leading edge.
    Left,
    /// Against the column's trailing edge.
    Right,
    /// Centred in the column.
    Center,
}

/// Text-wrap configuration for a floating object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FloatWrap {
    /// The wrap mode.
    pub wrap: TextWrap,
    /// Which side(s) text may occupy (meaningful for `Square`/`Tight`/`Through`).
    pub side: WrapSide,
    /// The object's own horizontal placement, when the producer stated one.
    ///
    /// `None` means unstated, and the side is then inferred from
    /// [`side`](Self::side) — which is all ODF and legacy content offers. An
    /// explicit value **wins**, because `wrapText="bothSides"` says nothing
    /// about where the object is: `acid2-docx.docx`'s newsletter figure is
    /// `bothSides` with `<wp:align>right</wp:align>`, and inferring from the
    /// wrap side alone put it on the left, mirror-imaging the page.
    pub align: Option<FloatAlign>,
    /// `true` when the object sits behind the text (OOXML `wp:wrapNone` with
    /// `behindDoc="1"`; ODF `style:run-through="background"`).
    pub behind_text: bool,
    /// Clearance the wrapped text must leave around the object, per side, when
    /// the producer stated it.
    ///
    /// `None` means unstated — ODF carries no equivalent today, and legacy
    /// content may omit it — and the layout then falls back to its own default
    /// gap. This mirrors [`align`](Self::align): an explicit value is document
    /// data and wins; absence is not the same as zero.
    pub dist: Option<WrapDistance>,
}

/// Clearance around a floating object, in **EMU** (914400 per inch).
///
/// OOXML `wp:anchor/@distT|@distB|@distL|@distR`, kept in the file's own unit so
/// the value is exact and the type stays `Eq` (as `FloatWrap` requires). Word
/// writes `114300` (9 pt) left/right and `45720` (3.6 pt) top/bottom by default,
/// but they are ordinary attributes and a producer may state anything, including
/// zero — which is why the layout must read them rather than assume a constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WrapDistance {
    /// Clearance above the object.
    pub top: i64,
    /// Clearance below the object.
    pub bottom: i64,
    /// Clearance to the object's left.
    pub left: i64,
    /// Clearance to the object's right.
    pub right: i64,
}

impl FloatWrap {
    /// Writes this wrap configuration into `attr`: ensures the [`FLOATING_CLASS`]
    /// class is present and records the wrap mode/side/behind flag in `kv`.
    /// Idempotent — any previously stored wrap keys are replaced.
    pub fn store(&self, attr: &mut NodeAttr) {
        if !attr.classes.iter().any(|c| c == FLOATING_CLASS) {
            attr.classes.push(FLOATING_CLASS.to_string());
        }
        attr.kv.retain(|(k, _)| {
            k != KV_WRAP
                && k != KV_SIDE
                && k != KV_BEHIND
                && k != KV_ALIGN
                && k != KV_DIST_T
                && k != KV_DIST_B
                && k != KV_DIST_L
                && k != KV_DIST_R
        });
        attr.kv
            .push((KV_WRAP.to_string(), self.wrap.as_kv().to_string()));
        attr.kv
            .push((KV_SIDE.to_string(), self.side.as_kv().to_string()));
        if self.behind_text {
            attr.kv.push((KV_BEHIND.to_string(), "true".to_string()));
        }
        // Written only when stated, so a round-trip cannot invent a placement
        // the producer never gave (see `align`).
        if let Some(align) = self.align {
            attr.kv
                .push((KV_ALIGN.to_string(), align.as_kv().to_string()));
        }
        // Same rule: only a stated clearance is written, so an unstated one
        // cannot come back as an explicit zero.
        if let Some(d) = self.dist {
            for (key, v) in [
                (KV_DIST_T, d.top),
                (KV_DIST_B, d.bottom),
                (KV_DIST_L, d.left),
                (KV_DIST_R, d.right),
            ] {
                attr.kv.push((key.to_string(), format!("{v}")));
            }
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
            align: None,
            behind_text: false,
            dist: None,
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
        let align = attr
            .kv
            .iter()
            .find(|(k, _)| k == KV_ALIGN)
            .and_then(|(_, v)| FloatAlign::from_kv(v));
        let read_dist = |key: &str| {
            attr.kv
                .iter()
                .find(|(k, _)| k == key)
                .and_then(|(_, v)| v.parse::<i64>().ok())
        };
        // Present iff at least one side was stored — `store` writes all four
        // together, so any one of them implies a stated clearance.
        let dist = [KV_DIST_T, KV_DIST_B, KV_DIST_L, KV_DIST_R]
            .iter()
            .any(|k| attr.kv.iter().any(|(kk, _)| kk == k))
            .then(|| WrapDistance {
                top: read_dist(KV_DIST_T).unwrap_or(0),
                bottom: read_dist(KV_DIST_B).unwrap_or(0),
                left: read_dist(KV_DIST_L).unwrap_or(0),
                right: read_dist(KV_DIST_R).unwrap_or(0),
            });
        Some(FloatWrap {
            wrap,
            side,
            align,
            behind_text,
            dist,
        })
    }
}

#[cfg(test)]
#[path = "float_tests.rs"]
mod tests;
