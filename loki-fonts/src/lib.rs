// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Bundled metric-compatible fallback fonts for the Android CPU renderer.
//!
//! | Bundled family | Covers           |
//! |----------------|------------------|
//! | Carlito        | Calibri          |
//! | Caladea        | Cambria          |
//! | Arimo          | Arial            |
//! | Cousine        | Courier New      |
//! | Tinos          | Times New Roman  |
//!
//! All fonts are licensed under SIL OFL 1.1 from <https://github.com/google/fonts>.
//!
//! # Usage
//!
//! ```ignore
//! // In the Dioxus App component — safe to call on all platforms:
//! document::Style { r#type: "text/css", "{loki_fonts::face_css()}" }
//! ```
//!
//! The raw face bytes ([`fallback_font_blobs`]) are embedded on **all** platforms
//! — the layout engine registers them lazily for metric-compatible substitution in
//! headless/CI/PDF-export contexts as well as Android. Only the `@font-face` CSS
//! generation ([`face_css`], used by the Android-CPU HTML fallback) is gated to
//! that target and returns `""` elsewhere.

#![forbid(unsafe_code)]

use std::sync::OnceLock;

/// Returns a self-contained `@font-face` block for the **Atkinson Hyperlegible
/// Next** UI variable font, embedded as a `data:font/truetype;base64,…` URI.
///
/// Embedded on **all** platforms (the single variable font is ~112 KB). Unlike
/// the `dioxus:///assets/...` URL the apps previously used — which resolves
/// relative to the executable and fails to load on Android/ChromeOS (and
/// silently relies on a system-installed copy on desktop) — the `data:` URI is
/// decoded by `blitz_net` on every platform, so the UI chrome renders in the
/// intended face everywhere. Built once and cached for the process lifetime.
pub fn ui_face_css() -> &'static str {
    static UI_FACE_CSS: OnceLock<String> = OnceLock::new();
    UI_FACE_CSS.get_or_init(|| {
        const FONT: &[u8] = include_bytes!("../fonts/AtkinsonHyperlegibleNext-VF.ttf");
        let b64 = base64_encode(FONT);
        format!(
            "@font-face{{font-family:'Atkinson Hyperlegible Next';\
             font-weight:100 900;font-style:normal;\
             src:url('data:font/truetype;base64,{b64}') format('truetype');}}"
        )
    })
}

/// Standard base64 encoder (no line wrapping), shared by [`ui_face_css`] and the
/// Android fallback-font CSS. Dependency-free to keep this crate `include_bytes`
/// only.
pub(crate) fn base64_encode(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Returns a complete `@font-face` CSS block for all bundled fonts, with each
/// font embedded as a `data:font/truetype;base64,…` URI.
///
/// Built once on first call and cached for the process lifetime.
/// Returns `""` on desktop and Android GPU builds.
// On Android CPU the cfg-gated return is always taken, making the fallback ""
// unreachable on that target.  The allow is intentional: "" IS reached on all
// other targets (desktop, Android GPU).
#[allow(unreachable_code)]
pub fn face_css() -> &'static str {
    #[cfg(all(target_os = "android", not(android_gpu)))]
    return imp::face_css_impl();

    ""
}

/// Raw bytes of every bundled metric-compatible fallback face (Carlito, Caladea,
/// Arimo, Cousine, Tinos), for direct registration into the document layout
/// engine's font collection.
///
/// Available on **all** platforms. The layout engine registers these lazily, only
/// when a substitute family (e.g. Carlito for Calibri) is requested but not found
/// in the collection — so a properly-installed desktop never pays for them, while
/// headless export, CI, and Android (where the executable-relative `assets/fonts/`
/// directory does not resolve) still resolve Calibri/Arial/Times to a
/// metric-compatible face instead of a wider system fallback.
pub fn fallback_font_blobs() -> &'static [&'static [u8]] {
    imp::fallback_font_blobs()
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;

    #[test]
    fn face_css_is_empty_on_non_android() {
        // Desktop and Android-GPU builds must not embed any @font-face CSS
        // (the ~7 MB of font bytes are android-cpu-only). This is the documented
        // no-op contract relied on by the shared App component.
        assert_eq!(face_css(), "");
    }
}

// ── Bundled font-face implementation ──────────────────────────────────────────
// The raw font bytes (`FACES` / `fallback_font_blobs`) are compiled on every
// platform so the layout engine can register metric-compatible substitutes in
// headless/CI/PDF-export contexts, not just Android. The ~7 MB embed is the cost
// of guaranteed Word-fidelity substitution. The `@font-face` CSS generation
// (`face_css_impl`/`build_css`) — used only by the Android-CPU HTML fallback —
// remains gated to that target.

mod imp {
    #[cfg(all(target_os = "android", not(android_gpu)))]
    use std::sync::OnceLock;

    // `family`/`weight`/`style` are read only by the Android CSS builder; on other
    // targets only `bytes` is used (by `fallback_font_blobs`).
    #[cfg_attr(not(all(target_os = "android", not(android_gpu))), allow(dead_code))]
    struct Face {
        family: &'static str,
        weight: &'static str, // "100 900" for variable fonts, "400"/"700" for static
        style: &'static str,  // "normal" | "italic"
        bytes: &'static [u8],
    }

    static FACES: &[Face] = &[
        // Arimo — metric-compatible Arial (variable font, covers all weights)
        Face {
            family: "Arimo",
            weight: "100 900",
            style: "normal",
            bytes: include_bytes!("../fonts/Arimo[wght].ttf"),
        },
        Face {
            family: "Arimo",
            weight: "100 900",
            style: "italic",
            bytes: include_bytes!("../fonts/Arimo-Italic[wght].ttf"),
        },
        // Caladea — metric-compatible Cambria
        Face {
            family: "Caladea",
            weight: "400",
            style: "normal",
            bytes: include_bytes!("../fonts/Caladea-Regular.ttf"),
        },
        Face {
            family: "Caladea",
            weight: "700",
            style: "normal",
            bytes: include_bytes!("../fonts/Caladea-Bold.ttf"),
        },
        Face {
            family: "Caladea",
            weight: "400",
            style: "italic",
            bytes: include_bytes!("../fonts/Caladea-Italic.ttf"),
        },
        Face {
            family: "Caladea",
            weight: "700",
            style: "italic",
            bytes: include_bytes!("../fonts/Caladea-BoldItalic.ttf"),
        },
        // Cousine — metric-compatible Courier New
        Face {
            family: "Cousine",
            weight: "400",
            style: "normal",
            bytes: include_bytes!("../fonts/Cousine-Regular.ttf"),
        },
        Face {
            family: "Cousine",
            weight: "700",
            style: "normal",
            bytes: include_bytes!("../fonts/Cousine-Bold.ttf"),
        },
        Face {
            family: "Cousine",
            weight: "400",
            style: "italic",
            bytes: include_bytes!("../fonts/Cousine-Italic.ttf"),
        },
        Face {
            family: "Cousine",
            weight: "700",
            style: "italic",
            bytes: include_bytes!("../fonts/Cousine-BoldItalic.ttf"),
        },
        // Tinos — metric-compatible Times New Roman
        Face {
            family: "Tinos",
            weight: "400",
            style: "normal",
            bytes: include_bytes!("../fonts/Tinos-Regular.ttf"),
        },
        Face {
            family: "Tinos",
            weight: "700",
            style: "normal",
            bytes: include_bytes!("../fonts/Tinos-Bold.ttf"),
        },
        Face {
            family: "Tinos",
            weight: "400",
            style: "italic",
            bytes: include_bytes!("../fonts/Tinos-Italic.ttf"),
        },
        Face {
            family: "Tinos",
            weight: "700",
            style: "italic",
            bytes: include_bytes!("../fonts/Tinos-BoldItalic.ttf"),
        },
        // Carlito — metric-compatible Calibri
        Face {
            family: "Carlito",
            weight: "400",
            style: "normal",
            bytes: include_bytes!("../fonts/Carlito-Regular.ttf"),
        },
        Face {
            family: "Carlito",
            weight: "700",
            style: "normal",
            bytes: include_bytes!("../fonts/Carlito-Bold.ttf"),
        },
        Face {
            family: "Carlito",
            weight: "400",
            style: "italic",
            bytes: include_bytes!("../fonts/Carlito-Italic.ttf"),
        },
        Face {
            family: "Carlito",
            weight: "700",
            style: "italic",
            bytes: include_bytes!("../fonts/Carlito-BoldItalic.ttf"),
        },
    ];

    #[cfg(all(target_os = "android", not(android_gpu)))]
    static FACE_CSS: OnceLock<String> = OnceLock::new();

    #[cfg(all(target_os = "android", not(android_gpu)))]
    pub(super) fn face_css_impl() -> &'static str {
        FACE_CSS.get_or_init(build_css)
    }

    /// Raw bytes of every bundled fallback face, for direct registration into a
    /// font collection (e.g. Parley's). Compiled on every platform; the layout
    /// engine registers them lazily when a metric-compatible substitute is
    /// requested but not already present in the collection.
    pub(super) fn fallback_font_blobs() -> &'static [&'static [u8]] {
        use std::sync::OnceLock;
        static BLOBS: OnceLock<Vec<&'static [u8]>> = OnceLock::new();
        BLOBS.get_or_init(|| FACES.iter().map(|f| f.bytes).collect())
    }

    #[cfg(all(target_os = "android", not(android_gpu)))]
    fn build_css() -> String {
        use std::fmt::Write as _;

        let total_bytes: usize = FACES.iter().map(|f| f.bytes.len()).sum();
        let mut css = String::with_capacity(total_bytes * 4 / 3 + FACES.len() * 256);
        for face in FACES {
            let b64 = crate::base64_encode(face.bytes);
            writeln!(
                css,
                "@font-face{{font-family:'{family}';font-weight:{weight};\
                 font-style:{style};src:url('data:font/truetype;base64,{b64}')\
                 format('truetype');}}",
                family = face.family,
                weight = face.weight,
                style = face.style,
            )
            .unwrap();
        }
        css
    }
}
