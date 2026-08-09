# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-05

### Added

- **Color value types**: `RgbColor`, `CmykColor`, `LabColor`, `XyzColor`, `GrayColor`
  with silent clamping constructors and immutable accessors
- **`ColorValue` enum**: Unified discriminated union over all supported color models
- **`ColorSpace` enum**: RGB, CMYK, Lab, XYZ, Gray with channel count and `moxcms`
  layout mapping
- **`RenderingIntent` enum**: Perceptual, Relative Colorimetric, Saturation,
  Absolute Colorimetric with `moxcms` round-trip conversion
- **`IccProfile`**: Wrapper around `moxcms::ColorProfile` with `from_bytes`,
  `from_path` (std), and built-in profile constructors (`new_srgb`, `new_adobe_rgb`,
  `new_display_p3`, `new_lab`, `new_gray`)
- **`ColorTransform`**: Builder-pattern profile-to-profile transform using `moxcms`
  f32 pipeline
- **`ProofingConfig`**: Two-stage soft proofing (source → simulation → display)
  with configurable intents and `GamutWarning` support
- **`ColorPolicy` and `OutputIntent`**: Format-agnostic document-level color
  management policy with embedded profile registry
- **`ColorError` and `ColorResult`**: Comprehensive error type with `thiserror`
  derives and actionable variants
- `#![forbid(unsafe_code)]` enforced at crate root
- `no_std` support via `std` feature flag (default enabled)
- Optional `serde` feature for color type serialization
- Full doc-comments with `# Errors` and `# Examples` on every public item
- 127 tests: 17 unit tests, 19 integration tests, 91 doc-tests
