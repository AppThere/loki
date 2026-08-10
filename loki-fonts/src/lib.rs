// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Bundled UI typeface and metric-compatible fallback fonts.
//!
//! | Bundled family             | Covers                  |
//! |----------------------------|-------------------------|
//! | Atkinson Hyperlegible Next | UI chrome typeface      |
//! | Carlito                    | Calibri                 |
//! | Caladea                    | Cambria                 |
//! | Arimo                      | Arial                   |
//! | Cousine                    | Courier New             |
//! | Tinos                      | Times New Roman         |
//! | Gelasio                    | Georgia                 |
//!
//! Atkinson Hyperlegible Next is licensed under SIL OFL 1.1 by the Braille
//! Institute; the metric-compatible faces under SIL OFL 1.1 from
//! <https://github.com/google/fonts>. The bundled Gelasio faces (added under
//! Spec 02 B-10; license in `fonts/OFL-Gelasio.txt`, no Reserved Font Name)
//! were reconstructed from the `@fontsource/gelasio` npm distribution by
//! merging its latin + latin-ext + vietnamese subsets with fonttools — the
//! full upstream coverage for this face.
//!
//! # Usage
//!
//! The fonts are registered **synchronously** into the renderer's font
//! collection at launch — there is no `@font-face` / `data:` URI step. The Dioxus
//! Native apps pass the bytes through the launch config:
//!
//! ```ignore
//! dioxus::native::launch_cfg(
//!     App,
//!     vec![],
//!     vec![Box::new(dioxus::native::Config::new().with_fonts(loki_fonts::ui_font_blobs()))],
//! );
//! ```
//!
//! This is the robust, platform-independent path: the family names resolve before
//! first paint on every platform, including Android, where the previous approach
//! (a `@font-face` `data:` URI decoded asynchronously by the renderer's network
//! provider) did not load the UI typeface — the chrome fell back to a wide system
//! font. Synchronous registration removes the dependency on that async path
//! entirely, which is the correct layer to fix: the bytes are known at compile
//! time, so there is no reason to fetch them at runtime.
//!
//! The raw face bytes are also exposed via [`fallback_font_blobs`] for the
//! document layout engine, which registers them lazily for metric-compatible
//! substitution in headless/CI/PDF-export contexts.

#![forbid(unsafe_code)]

/// The Atkinson Hyperlegible Next UI variable font, embedded on every platform.
const ATKINSON_VF: &[u8] = include_bytes!("../fonts/AtkinsonHyperlegibleNext-VF.ttf");

/// Raw bytes of every bundled UI/fallback face, for **synchronous** registration
/// into the renderer's Parley `FontContext` at launch.
///
/// Includes the Atkinson Hyperlegible Next UI variable font followed by the six
/// metric-compatible fallback families (see [`fallback_font_blobs`]). Registering
/// these at startup makes the family names ("Atkinson Hyperlegible Next",
/// "Carlito", "Caladea", "Arimo", "Cousine", "Tinos", "Gelasio") resolve immediately on
/// every platform, without relying on the asynchronous `@font-face` `data:` URI
/// fetch (which is unreliable on Android).
pub fn ui_font_blobs() -> Vec<Vec<u8>> {
    let mut blobs = Vec::with_capacity(1 + fallback_font_blobs().len());
    blobs.push(ATKINSON_VF.to_vec());
    blobs.extend(fallback_font_blobs().iter().map(|b| b.to_vec()));
    blobs
}

/// Raw bytes of every bundled metric-compatible fallback face (Carlito, Caladea,
/// Arimo, Cousine, Tinos, Gelasio), for direct registration into the document
/// layout engine's font collection.
///
/// Available on **all** platforms. The layout engine registers these lazily, only
/// when a substitute family (e.g. Carlito for Calibri) is requested but not found
/// in the collection — so a properly-installed desktop never pays for them, while
/// headless export, CI, and Android (where the executable-relative `assets/fonts/`
/// directory does not resolve) still resolve Calibri/Arial/Times to a
/// metric-compatible face instead of a wider system fallback.
pub fn fallback_font_blobs() -> &'static [&'static [u8]] {
    /// The bundled metric-compatible faces. Each `include_bytes!` is coerced to
    /// `&[u8]` so the array unifies despite differing lengths.
    static FACES: &[&[u8]] = &[
        // Arimo — metric-compatible Arial (variable font, covers all weights)
        include_bytes!("../fonts/Arimo[wght].ttf"),
        include_bytes!("../fonts/Arimo-Italic[wght].ttf"),
        // Caladea — metric-compatible Cambria
        include_bytes!("../fonts/Caladea-Regular.ttf"),
        include_bytes!("../fonts/Caladea-Bold.ttf"),
        include_bytes!("../fonts/Caladea-Italic.ttf"),
        include_bytes!("../fonts/Caladea-BoldItalic.ttf"),
        // Cousine — metric-compatible Courier New
        include_bytes!("../fonts/Cousine-Regular.ttf"),
        include_bytes!("../fonts/Cousine-Bold.ttf"),
        include_bytes!("../fonts/Cousine-Italic.ttf"),
        include_bytes!("../fonts/Cousine-BoldItalic.ttf"),
        // Tinos — metric-compatible Times New Roman
        include_bytes!("../fonts/Tinos-Regular.ttf"),
        include_bytes!("../fonts/Tinos-Bold.ttf"),
        include_bytes!("../fonts/Tinos-Italic.ttf"),
        include_bytes!("../fonts/Tinos-BoldItalic.ttf"),
        // Carlito — metric-compatible Calibri
        include_bytes!("../fonts/Carlito-Regular.ttf"),
        include_bytes!("../fonts/Carlito-Bold.ttf"),
        include_bytes!("../fonts/Carlito-Italic.ttf"),
        include_bytes!("../fonts/Carlito-BoldItalic.ttf"),
        // Gelasio — metric-compatible Georgia (Spec 02 B-10)
        include_bytes!("../fonts/Gelasio-Regular.ttf"),
        include_bytes!("../fonts/Gelasio-Bold.ttf"),
        include_bytes!("../fonts/Gelasio-Italic.ttf"),
        include_bytes!("../fonts/Gelasio-BoldItalic.ttf"),
    ];
    FACES
}

/// A bundled document face and the proprietary family it stands in for.
///
/// The font picker sorts these first and badges them, because a bundled face is
/// the only kind that **renders identically for every reader** — it ships with
/// Loki, embeds into EPUB, and is the one class of face with no substitution
/// risk. A device font may look right on the author's machine and reflow on
/// someone else's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BundledFamily {
    /// The family name as the layout engine resolves it (e.g. `"Tinos"`).
    pub name: &'static str,
    /// The proprietary family it is metric-compatible with (e.g.
    /// `"Times New Roman"`) — the answer to "which one is Arial?".
    pub substitutes_for: &'static str,
}

/// Every bundled document family, sorted by [`BundledFamily::name`].
///
/// This is the **naming** counterpart of [`fallback_font_blobs`] and is
/// deliberately its neighbour: the two describe the same set, and splitting the
/// list across crates is how they drift. The UI typeface (Atkinson Hyperlegible
/// Next) is not here — it is chrome, never offered as a document face.
#[must_use]
pub fn bundled_families() -> &'static [BundledFamily] {
    &[
        BundledFamily {
            name: "Arimo",
            substitutes_for: "Arial",
        },
        BundledFamily {
            name: "Caladea",
            substitutes_for: "Cambria",
        },
        BundledFamily {
            name: "Carlito",
            substitutes_for: "Calibri",
        },
        BundledFamily {
            name: "Cousine",
            substitutes_for: "Courier New",
        },
        BundledFamily {
            name: "Gelasio",
            substitutes_for: "Georgia",
        },
        BundledFamily {
            name: "Tinos",
            substitutes_for: "Times New Roman",
        },
    ]
}

/// Whether `family` is bundled with Loki (case-insensitive).
///
/// Case-insensitive because the name arrives from a document that may have been
/// written anywhere: ODF and OOXML both carry the family as free text, so
/// `TINOS` and `tinos` are the same face and must badge as bundled.
#[must_use]
pub fn is_bundled_family(family: &str) -> bool {
    let family = family.trim();
    bundled_families()
        .iter()
        .any(|f| f.name.eq_ignore_ascii_case(family))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The picker renders the list as-is, so the order is part of the contract.
    #[test]
    fn bundled_families_are_sorted_by_name() {
        let names: Vec<&str> = bundled_families().iter().map(|f| f.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }

    /// The name list and the blob list describe the same set. A face added to
    /// one and not the other is exactly the drift this pairing exists to catch,
    /// so the check is mechanical rather than a comment.
    #[test]
    fn every_named_family_has_bundled_faces() {
        // Two Arimo blobs (variable roman + italic); four for each static face.
        let expected = 2 + (bundled_families().len() - 1) * 4;
        assert_eq!(
            fallback_font_blobs().len(),
            expected,
            "bundled_families() and fallback_font_blobs() disagree on the face set"
        );
    }

    /// A bundled face stands in for something; an empty string would badge in
    /// the picker with nothing to say.
    #[test]
    fn every_family_names_the_face_it_replaces() {
        for f in bundled_families() {
            assert!(!f.name.is_empty());
            assert!(!f.substitutes_for.is_empty(), "{} names no face", f.name);
            assert_ne!(f.name, f.substitutes_for);
        }
    }

    /// Family names arrive from documents written anywhere, so the check cannot
    /// be case-sensitive — and must still reject a face that is not bundled.
    #[test]
    fn membership_ignores_case_and_padding_but_not_identity() {
        for cased in ["Tinos", "tinos", "TINOS", "  Tinos  "] {
            assert!(is_bundled_family(cased), "{cased} is bundled");
        }
        for absent in ["Times New Roman", "Helvetica Neue", "Noto Sans", ""] {
            assert!(!is_bundled_family(absent), "{absent} is not bundled");
        }
    }

    // Regression guard: the embedded metric-compatible faces must be available on
    // every platform (not gated to Android), so headless/CI/PDF-export builds can
    // register them.
    #[test]
    fn fallback_font_blobs_embedded_on_all_targets() {
        assert!(
            !fallback_font_blobs().is_empty(),
            "metric-compatible fallback faces must be embedded on this target"
        );
    }

    // `ui_font_blobs` must carry the UI typeface plus every fallback face, and no
    // blob may be empty (an empty blob would silently fail to register).
    #[test]
    fn ui_font_blobs_includes_ui_face_plus_fallbacks() {
        let blobs = ui_font_blobs();
        assert_eq!(
            blobs.len(),
            1 + fallback_font_blobs().len(),
            "ui_font_blobs must be the UI face followed by every fallback face"
        );
        assert!(
            blobs.iter().all(|b| !b.is_empty()),
            "no bundled font blob may be empty"
        );
    }
}
