# appthere-color

**Document-aware color management for Rust — pure Rust ICC transforms, CMYK, soft proofing, and print-ready color policies.**

---

`appthere-color` fills a gap in the Rust ecosystem: a color model built for **document authoring and print workflows**, rather than pixel rendering. It provides named color values, CMYK ink representation, ICC profile transforms via a pure-Rust backend ([moxcms](https://crates.io/crates/moxcms)), rendering intent selection, soft proofing configuration, and output intent policies — all the primitives a document editor, prepress tool, or print-aware renderer needs.

---

## Features

- **Pure Rust ICC transforms** via `moxcms` — no C toolchain, no `liblcms2` system dependency, cross-compiles cleanly to Android and WASM targets
- **CMYK and multi-ink color values** with f32 component precision
- **Named color spaces** — sRGB, Display P3, Adobe RGB (1998), ProPhoto RGB, CMYK (generic and profile-bound)
- **Rendering intents** — Perceptual, Relative Colorimetric, Saturation, Absolute Colorimetric
- **Soft proofing configuration** — proof profile, simulation intent, gamut warning, paper simulation
- **Document color policy** — named output intent, fallback profile, per-document color state
- **Profile-to-profile transforms** — CMYK↔RGB, RGB↔RGB, Lab↔RGB, Gray↔RGB
- `no_std` compatible (with `moxcms` backend, disable default features)

---

## Installation

```toml
[dependencies]
appthere-color = "0.1"
```

No system libraries required. Everything is pure Rust.

---

## Quick Start

### Color values

```rust
use appthere_color::{ColorValue, CmykColor, RgbColor};

// RGB color (0.0–1.0 components)
let red = ColorValue::Rgb(RgbColor::new(1.0, 0.0, 0.0));

// CMYK ink values (0.0–1.0 per channel)
let rich_black = ColorValue::Cmyk(CmykColor::new(0.60, 0.50, 0.50, 1.0));

// Lab color (D50 whitepoint)
let lab = ColorValue::Lab { l: 53.39, a: 80.09, b: 67.20 };
```

### ICC profile transforms

```rust
use appthere_color::{IccProfile, ColorTransform, RenderingIntent};

let fogra39 = IccProfile::from_bytes(include_bytes!("profiles/ISOcoated_v2_300_eci.icc"))?;
let srgb = IccProfile::srgb();

let transform = ColorTransform::new(&fogra39, &srgb, RenderingIntent::RelativeColorimetric)?;

let cmyk_pixel = [0.60f32, 0.50, 0.50, 1.0];
let rgb_pixel = transform.transform_cmyk_to_rgb(&cmyk_pixel)?;
```

### Soft proofing

```rust
use appthere_color::{ProofingConfig, GamutWarning, RenderingIntent};

let config = ProofingConfig::builder()
    .output_profile(fogra39)
    .simulation_intent(RenderingIntent::RelativeColorimetric)
    .display_profile(srgb)
    .gamut_warning(GamutWarning::Color([1.0, 0.0, 1.0])) // magenta flag
    .paper_simulation(true)
    .build()?;

let proofing_transform = config.build_transform()?;
```

### Document color policy

```rust
use appthere_color::{ColorPolicy, OutputIntent};

let policy = ColorPolicy::builder()
    .output_intent(OutputIntent::named("FOGRA39", fogra39_profile))
    .default_rendering_intent(RenderingIntent::Perceptual)
    .fallback_profile(IccProfile::srgb())
    .build();

// Serialize the policy into your document format's color metadata
```

---

## Color Value Types

| Type | Channels | Range | Notes |
|---|---|---|---|
| `RgbColor` | R, G, B | 0.0–1.0 | Linear or gamma-encoded depending on profile |
| `CmykColor` | C, M, Y, K | 0.0–1.0 | Profile-bound ink percentages |
| `LabColor` | L, a, b | L: 0–100, a/b: ±128 | D50 whitepoint |
| `XyzColor` | X, Y, Z | 0.0–1.0 (relative) | D50 |
| `GrayColor` | K | 0.0–1.0 | |
| `ColorValue` | — | — | Enum over all of the above |

---

## Rendering Intents

```rust
pub enum RenderingIntent {
    Perceptual,
    RelativeColorimetric,
    Saturation,
    AbsoluteColorimetric,
}
```

These map directly to ICC specification intents and are passed through to the `moxcms` transform engine without modification.

---

## Soft Proofing

Soft proofing simulates how a document will appear when reproduced on a specific output device (typically a press or printer). `appthere-color` models proofing as a first-class configuration object rather than a transform flag:

```rust
pub struct ProofingConfig {
    pub output_profile: IccProfile,
    pub simulation_intent: RenderingIntent,
    pub display_profile: IccProfile,
    pub display_intent: RenderingIntent,
    pub gamut_warning: GamutWarning,
    pub paper_simulation: bool,
}
```

Soft proofing is implemented as two chained transforms internally: source → output simulation space, then simulation space → display. This is semantically equivalent to LCMS2's proofing transform and produces the same result for standard workflows.

### Gamut warning modes

```rust
pub enum GamutWarning {
    /// Do not flag out-of-gamut colors
    None,
    /// Replace out-of-gamut colors with a fixed indicator color
    Color([f32; 3]),
    /// Reduce saturation of out-of-gamut colors by a factor
    Desaturate(f32),
}
```

---

## ICC Profile Support

Profiles are loaded from raw ICC bytes and backed by `moxcms`:

```rust
// From bytes (e.g. embedded in a document or file)
let profile = IccProfile::from_bytes(&icc_bytes)?;

// From a file path
let profile = IccProfile::from_file("path/to/profile.icc")?;

// Built-in virtual profiles
let srgb   = IccProfile::srgb();
let linear = IccProfile::linear_srgb();
let gray   = IccProfile::gray_d50();
```

Supported transform combinations mirror `moxcms` capabilities: CMYK↔RGB, RGB↔RGB, Lab↔RGB, Gray↔RGB, and any Display Class profiles up to 16 inks.

---

## Document Color Policy

A `ColorPolicy` captures the color management configuration for an entire document — the intended output condition, rendering intent, and fallback behavior when profiles are unavailable. This concept appears in PDF/X (OutputIntent), ODF (`draw:color-profile`), and EPUB (embedded ICC in images), but `appthere-color` represents it format-agnostically:

```rust
pub struct ColorPolicy {
    pub output_intent: Option<OutputIntent>,
    pub default_rendering_intent: RenderingIntent,
    pub fallback_profile: IccProfile,
    pub embedded_profiles: HashMap<String, IccProfile>,
}
```

Format-specific serialization (e.g. writing an `/OutputIntents` array into a PDF or the color profile metadata into an ODF package) is the responsibility of the document format crate that depends on `appthere-color`.

---

## Why not `lcms2`?

[`lcms2`](https://crates.io/crates/lcms2) is an excellent crate wrapping the mature Little CMS 2 library. If you need spot color (`NamedColorList`), DeviceLink profiles, or maximum compatibility with exotic real-world ICC profiles, it may be the right choice.

`appthere-color` uses `moxcms` instead because:

- **No C toolchain required** — clean cross-compilation to Android, WASM, and musl targets without a system `liblcms2`
- **Pure Rust safety guarantees** — no `unsafe` FFI surface
- **Simpler CI** — no need to manage native library availability across platforms

The tradeoff: `moxcms` does not yet support spot/named color lookups or some exotic DeviceLink profile classes. For document authoring workflows targeting standard RGB and CMYK output conditions, this is not a practical limitation.

---

## `no_std` Support

`appthere-color` is `no_std` compatible. Disable the default `std` feature:

```toml
[dependencies]
appthere-color = { version = "0.1", default-features = false }
```

ICC file loading from paths requires `std`. Profile construction from `&[u8]` and all transform operations work in `no_std` environments.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

`moxcms` is also Apache 2.0. The full dependency tree is permissively licensed.

---

## Contributing

Issues and pull requests are welcome. The scope of this crate is intentionally narrow — document-authoring color primitives and ICC transforms. Requests for PDF/ODF/EPUB serialization belong in the respective format crates.