# GEMINI.md — appthere-color

This file provides project context and coding standards for AI-assisted development on the `appthere-color` crate. Read this before making any changes.

---

## Project Purpose

`appthere-color` is a **document-authoring color management library** for Rust. It was extracted from AppThere Loki (a cross-platform office suite built with Tauri 2 + React + Rust) and published as a standalone crate because the color model is broadly applicable and the Rust ecosystem lacks a document-authoring-focused color library.

It is used by:
- **AppThere Loki** — color management for ODF/OOXML documents and PDF/X export pipelines
- **AppThere Iris** — color accuracy in WASM/WebGPU raster image editing (where `lcms2` FFI is impractical)

### What this crate IS

- Color value types: RGB, CMYK, Lab, XYZ, Gray, and a `ColorValue` enum over all of them
- ICC profile loading and transform execution via `moxcms` (pure Rust backend)
- Rendering intent selection (Perceptual, Relative Colorimetric, Saturation, Absolute Colorimetric)
- Soft proofing configuration and chained transform execution
- Document color policy: output intent, fallback profile, embedded profile registry
- `no_std` compatible (profile loading from paths requires `std`)

### What this crate IS NOT

- It has **zero knowledge of document formats** — no ODF, OOXML, EPUB, or PDF serialization
- It is **not** a general-purpose pixel rendering color crate (see Linebender's `color` for that)
- It does **not** wrap `lcms2` or any C library — `moxcms` is the exclusive ICC backend
- It does **not** handle spot/named colors (Pantone etc.) — `moxcms` does not support these

### ICC Backend Choice

The crate uses [`moxcms`](https://crates.io/crates/moxcms) exclusively instead of `lcms2`:
- No C toolchain required — cross-compiles cleanly to Android, WASM, and musl targets
- Pure Rust: no `unsafe` FFI surface
- `moxcms` covers all transform combinations needed for document authoring: CMYK↔RGB, RGB↔RGB, Lab↔RGB, Gray↔RGB, Display Class ICC profiles up to 16 inks

Known `moxcms` limitations compared to `lcms2`:
- No `NamedColorList` (spot color lookups)
- No DeviceLink or Abstract profile class support
- Newer crate — less battle-tested against exotic real-world ICC profiles

Do not add `lcms2` as a dependency or introduce a feature flag for it without a documented architectural decision.

---

## Repository Layout

```
appthere-color/
├── src/
│   ├── lib.rs              # Public API surface, re-exports, crate-level docs
│   ├── color_value.rs      # ColorValue enum, RgbColor, CmykColor, LabColor, etc.
│   ├── color_space.rs      # ColorSpace enum and named space constants
│   ├── rendering_intent.rs # RenderingIntent enum
│   ├── profile.rs          # IccProfile wrapper around moxcms
│   ├── transform.rs        # ColorTransform: profile-to-profile conversion
│   ├── proofing.rs         # ProofingConfig, GamutWarning, chained proofing transforms
│   ├── policy.rs           # ColorPolicy, OutputIntent, document-level color state
│   └── error.rs            # ColorError, ColorResult
├── tests/
│   ├── transform_tests.rs
│   ├── proofing_tests.rs
│   └── policy_tests.rs
├── profiles/               # Test ICC profiles (sRGB, FOGRA39, ISOcoated)
├── benches/
│   └── transform_bench.rs
├── CLAUDE.md
├── GEMINI.md
├── README.md
├── Cargo.toml
└── LICENSE
```

---

## Coding Standards

These standards apply to all code in this repository. They are not suggestions.

### 1. File Length: 300-Line Hard Ceiling

**No source file may exceed 300 lines.** This is an absolute limit.

When a file approaches 300 lines:
- Identify logical sub-concerns and extract them into new files
- Use `mod` declarations and `pub use` re-exports to maintain the public API surface
- Prefer many small, focused files over fewer large ones

If completing a task would push any file over 300 lines, split the file first, then complete the task.

### 2. No `.unwrap()` or `.expect()` in Library Code

**`.unwrap()` and `.expect()` are forbidden in `src/`.** No exceptions.

All fallible operations must propagate errors using `?` and return `ColorResult<T>`. The caller decides how to handle failures — this library never panics on bad input.

```rust
// FORBIDDEN
let profile = IccProfile::from_bytes(&bytes).unwrap();
let value = map.get("key").expect("key must exist");

// CORRECT
let profile = IccProfile::from_bytes(&bytes)?;
let value = map.get("key").ok_or(ColorError::MissingProfile("key".into()))?;
```

`.unwrap()` is permitted only in `tests/` and `benches/`, where a panic is an acceptable test failure signal.

### 3. No Unsafe Code

**`unsafe` blocks are forbidden throughout this crate** — in `src/`, `tests/`, and `benches/`.

The crate's entire value proposition includes being a pure-safe-Rust library. `#![forbid(unsafe_code)]` is set at the crate root in `lib.rs` and must not be removed or overridden.

If you believe `unsafe` is required for a specific optimization, document the case and wait for discussion. The answer will almost always be to find a safe alternative.

### 4. Error Handling

All errors flow through `ColorError` in `error.rs`. Use `thiserror` for all error derivation. Do not write manual `Display` implementations for error variants.

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum ColorError {
    #[error("failed to parse ICC profile: {0}")]
    ProfileParse(String),

    #[error("transform requires compatible profile classes, got {src} and {dst}")]
    IncompatibleProfiles { src: String, dst: String },

    #[error("color component {component} out of range: {value}")]
    ComponentOutOfRange { component: &'static str, value: f32 },

    #[error("proofing config is incomplete: {0}")]
    IncompleteProofingConfig(String),
}

pub type ColorResult<T> = Result<T, ColorError>;
```

Error variants must be specific enough to be actionable. Avoid catch-all variants like `Other(String)` except as a last resort for upstream errors that cannot be mapped.

### 5. Test Coverage

Every public function and method must have tests. Tests live in `tests/` for integration tests or in `#[cfg(test)]` modules within source files for unit tests.

Required for each public item:
- **Happy path** — correct input produces correct output
- **Error paths** — invalid input returns the correct `ColorError` variant, never panics
- **Boundary conditions** — minimum/maximum component values, empty inputs, malformed ICC bytes
- **Round-trip accuracy** — transform pairs (e.g. sRGB→CMYK→sRGB) must verify the round-trip delta is within a documented tolerance (typically ≤ 1/255 per channel for 8-bit equivalence)

Test ICC profiles are committed to `profiles/` in the repository. Tests must not fetch profiles from the network or depend on system-installed ICC profiles.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmyk_components_clamp_on_construction() {
        let color = CmykColor::new(1.5, -0.1, 0.5, 0.0);
        assert_eq!(color.cyan(), 1.0);
        assert_eq!(color.magenta(), 0.0);
    }

    #[test]
    fn profile_from_empty_bytes_returns_error() {
        let result = IccProfile::from_bytes(&[]);
        assert!(matches!(result, Err(ColorError::ProfileParse(_))));
    }
}
```

### 6. Documentation Comments

**Every public item must have a `///` doc comment.** `#![warn(missing_docs)]` is set at the crate root. `cargo doc` must build with zero warnings.

#### What must be documented

- Every `pub struct`, `pub enum`, `pub trait`
- Every `pub fn` and public method
- Every variant of a `pub enum`
- Every `pub` field that is not self-evident from its name
- Every feature flag (documented in `lib.rs` module-level docs)

#### Doc comment structure

```rust
/// Short one-line summary ending with a period.
///
/// Longer explanation if behaviour is non-obvious. Describe *what* this
/// does, not *how*. Include invariants the caller must uphold.
///
/// # Errors
///
/// Document every `ColorError` variant this function can return and the
/// condition that triggers it. Required on all functions returning `ColorResult`.
///
/// # Examples
///
/// ```rust
/// use appthere_color::{CmykColor, ColorValue};
///
/// let ink = CmykColor::new(0.0, 0.0, 0.0, 1.0);
/// assert_eq!(ink.key(), 1.0);
/// ```
pub fn example() { }
```

The `# Errors` section is **required** on every function returning `ColorResult<T>`. The `# Examples` section is **required** on every public function and method. Examples are compiled and run as doc-tests — they must pass.

#### Module-level docs

Every `src/*.rs` file must open with a `//!` comment stating the module's single responsibility in one or two sentences.

#### Doc-test hygiene

- Close fallible examples with `# Ok::<(), appthere_color::ColorError>(())`
- Hide boilerplate imports with a leading `#`: `# use appthere_color::IccProfile;`
- Do not use `# ignore` or `# no_run` unless the example genuinely cannot run in CI — if used, explain why in a comment

#### Generating documentation

Build and open the full crate documentation:

```sh
cargo doc --no-deps --open
```

Run doc-tests only:

```sh
cargo test --doc
```

Check for missing docs and broken intra-doc links (the canonical CI check — must pass with zero warnings):

```sh
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

Inspect private items during development:

```sh
cargo doc --no-deps --document-private-items --open
```

### 7. API Design Principles

**Builder pattern for configuration types.** `ProofingConfig`, `ColorPolicy`, and `ColorTransform` are constructed via builders. Builders validate inputs at `.build()` time and return `ColorResult`.

**Component ranges.** All color components are `f32`:
- RGB, CMYK, Gray: `0.0..=1.0`
- Lab L: `0.0..=100.0`
- Lab a, b: `-128.0..=127.0`
- XYZ: `0.0..=1.0` (relative, D50)

Value type constructors clamp silently. Transform inputs validate and return `Err` on out-of-range.

**Immutability.** Color value types (`RgbColor`, `CmykColor`, `LabColor`, etc.) are immutable after construction.

**Derive order.** Use this consistent order: `#[derive(Debug, Clone, PartialEq)]`. Add `Copy` for small enums and value types (e.g. `RenderingIntent`, `GamutWarning`).

**Visibility.** Keep internal implementation details private. Only expose what is genuinely needed by callers. Prefer `pub(crate)` over `pub` for shared internals.

### 8. Dependencies

The permitted dependency list is intentionally minimal:

| Crate | Purpose | Feature-gated? |
|---|---|---|
| `moxcms` | ICC profile loading and pixel transforms | No |
| `thiserror` | Error derive macros | No |
| `serde` | Serialization of color types | Yes — `serde` feature |

**Do not add:**
- `lcms2` or `lcms2-sys` — banned; C toolchain dependency defeats the crate's purpose
- `palette` — overlapping scope; ecosystem licensing concerns
- `image` — brings in format dependencies unrelated to color management
- Any crate that is not Apache-2.0 or MIT licensed

New dependencies require justification in the PR description. When in doubt, do not add the dependency.

### 9. `no_std` Compatibility

The crate must remain `no_std` compatible. All code that requires `std` must be gated:

```rust
#[cfg(feature = "std")]
use std::path::Path;
```

The default feature set enables `std`. Do not use `std::collections::HashMap` in `no_std`-compatible code — use `BTreeMap` or gate it.

### 10. Cargo.toml Metadata

Maintain these fields accurately at all times:

```toml
[package]
name = "appthere-color"
version = "x.y.z"
edition = "2024"
license = "Apache-2.0"
description = "Document-aware color management for Rust — pure Rust ICC transforms, CMYK, soft proofing, and print-ready color policies"
repository = "https://github.com/appthere/appthere-color"
keywords = ["color", "icc", "cmyk", "color-management", "print"]
categories = ["graphics", "multimedia"]
```

The crate targets Rust edition 2024. Do not change this to 2021. Edition 2024 is stable as of Rust 1.85 and is the correct choice for new crates with no legacy MSRV constraint.

---

## Soft Proofing Implementation

Soft proofing simulates how a document will appear when printed on a specific output device. It is implemented as two chained `moxcms` transforms:

**Transform 1:** Source profile → Output/simulation profile (`simulation_intent`)
**Transform 2:** Output/simulation profile → Display profile (`display_intent`, typically Relative Colorimetric)

Gamut warning is applied between the two transforms: pixels that fall outside the output profile gamut are replaced with a warning color or desaturated before the display transform runs.

This is semantically equivalent to `lcms2`'s `Transform::new_proofing()` for standard workflows. The intermediate buffer between the two transforms is an implementation detail and must not be exposed in the public API.

---

## Color Policy: Scope and Boundaries

`ColorPolicy` is a format-agnostic representation of document-level color management intent. It maps to:
- **PDF/X**: `/OutputIntents` array and `DestOutputProfile`
- **ODF**: `draw:color-profile` in document styles
- **EPUB**: ICC profiles embedded in image resources

`appthere-color` provides the data model only. Format-specific serialization belongs in the consuming crate (AppThere Loki's format modules). Never import or reference ODF, OOXML, EPUB, or PDF structures from within this crate.

---

## Relationship to AppThere Loki

During development, `appthere-color` is consumed as a path dependency from AppThere Loki:

```toml
# Local development in AppThere Loki
appthere-color = { path = "../appthere-color" }
```

API breaking changes (any change to public types, function signatures, or error variants) require:
1. A minor version bump (`0.x` → `0.(x+1)`)
2. A simultaneous coordinated update to AppThere Loki's color service layer
3. An entry in `CHANGELOG.md`

Do not publish a breaking version to crates.io without first verifying AppThere Loki compiles against it.

---

## Quick Reference: What to Check Before Submitting

```
[ ] No file in src/ exceeds 300 lines
[ ] No .unwrap() or .expect() anywhere in src/
[ ] No unsafe blocks anywhere in the crate
[ ] Every new public item has a /// doc comment
[ ] Every public fn has # Errors (if fallible) and # Examples sections
[ ] Doc-tests compile and pass: cargo test --doc
[ ] RUSTDOCFLAGS="-D warnings" cargo doc --no-deps passes with zero warnings
[ ] Every new public item has at least one unit or integration test
[ ] Error paths have tests asserting the correct ColorError variant
[ ] No new dependencies added without justification
[ ] CHANGELOG.md updated
[ ] cargo test passes
[ ] cargo clippy -- -D warnings passes
```
