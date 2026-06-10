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
//! [`face_css`] returns `""` on desktop and Android GPU builds (no-op).
//! Font bytes are only embedded on `target_os = "android"`.

#![forbid(unsafe_code)]

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

// ── Android-only implementation ───────────────────────────────────────────────
// The entire font-data and CSS-generation block is compiled only on Android so
// that desktop binaries do not embed ~7 MB of font bytes.

#[cfg(target_os = "android")]
mod imp {
    use std::sync::OnceLock;

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

    static FACE_CSS: OnceLock<String> = OnceLock::new();

    pub(super) fn face_css_impl() -> &'static str {
        FACE_CSS.get_or_init(build_css)
    }

    fn build_css() -> String {
        use std::fmt::Write as _;

        let total_bytes: usize = FACES.iter().map(|f| f.bytes.len()).sum();
        let mut css = String::with_capacity(total_bytes * 4 / 3 + FACES.len() * 256);
        for face in FACES {
            let b64 = base64_encode(face.bytes);
            write!(
                css,
                "@font-face{{font-family:'{family}';font-weight:{weight};\
                 font-style:{style};src:url('data:font/truetype;base64,{b64}')\
                 format('truetype');}}\n",
                family = face.family,
                weight = face.weight,
                style = face.style,
            )
            .unwrap();
        }
        css
    }

    fn base64_encode(input: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
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
}
