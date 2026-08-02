# CLAUDE.md — appthere-color

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

The crate uses [`moxcms`](https://crates.io/crates/moxcms) exclusively instead of `lcms2` for these reasons:
- No C toolchain required — cross-compiles cleanly to Android, WASM, and musl targets
- Pure Rust: no `unsafe` FFI surface
- `moxcms` covers all transform combinations needed for document authoring: CMYK↔RGB, RGB↔RGB, Lab↔RGB, Gray↔RGB, Display Class ICC profiles up to 16 inks

Known `moxcms` limitations compared to `lcms2`:
- No `NamedColorList` (spot color lookups)
- No DeviceLink or Abstract profile class support
- Newer crate — less battle-tested against exotic real-world ICC profiles

Do not add `lcms2` as a dependency or introduce a feature flag for it without a documented architectural decision (ADR).

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

These are non-negotiable. Every contribution — human or AI-assisted — must comply.

### 1. File Length: 300-Line Hard Ceiling

**No source file may exceed 300 lines.** This is an absolute limit, not a guideline.

When a file approaches 300 lines:
- Identify logical sub-concerns and split them into new files
- Use `mod` declarations in the parent to re-export publicly where needed
- Prefer many small, focused files over fewer large ones

If you are adding code that would push a file over 300 lines, **stop and split first**.

### 2. No `.unwrap()` or `.expect()` in Library Code

**`.unwrap()` and `.expect()` are banned in `src/`.** No exceptions.

All fallible operations must return `Result<T, ColorError>` or `Option<T>` and propagate errors with `?`. The caller decides how to handle failures — this library never panics on bad input.

```rust
// BANNED
let profile = IccProfile::from_bytes(&bytes).unwrap();

// CORRECT
let profile = IccProfile::from_bytes(&bytes)?;
```

`.unwrap()` is permitted in `tests/` and `benches/` only, where a panic is an acceptable test failure signal.

### 3. No Unsafe Code

**`unsafe` is banned throughout this crate**, including in `src/`, `tests/`, and `benches/`.

This crate's entire value proposition includes being a pure-safe-Rust library. If you believe `unsafe` is required for a specific optimization, open a discussion first — the answer is almost certainly to find a safe alternative or accept the performance cost.

`#![forbid(unsafe_code)]` is set at the crate root in `lib.rs`. Do not remove it.

### 4. Error Handling

All errors flow through `ColorError` defined in `error.rs`. The error type must implement `std::error::Error`, `Display`, `Debug`, and `Clone`.

Use `thiserror` for deriving error implementations. Do not write manual `Display` impls for error variants — use `#[error("...")]` attributes.

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

### 5. Test Coverage

Every public function and method must have at least one test. Tests live in `tests/` (integration) or in `#[cfg(test)]` modules within `src/` files (unit).

Required coverage:
- **Happy path** — correct input produces correct output
- **Error paths** — invalid input produces the correct `ColorError` variant, not a panic
- **Boundary conditions** — color components at 0.0, 1.0, and out-of-range values
- **Round-trip accuracy** — for transform pairs (e.g. sRGB→CMYK→sRGB), verify the round-trip error is within a documented tolerance

Test ICC profiles are committed to `profiles/` in the repository. Tests must not fetch profiles from the network or depend on system-installed profiles.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmyk_components_are_clamped_on_construction() {
        let color = CmykColor::new(1.5, -0.1, 0.5, 0.0);
        assert_eq!(color.cyan(), 1.0);
        assert_eq!(color.magenta(), 0.0);
    }

    #[test]
    fn from_bytes_errors_on_empty_slice() {
        let result = IccProfile::from_bytes(&[]);
        assert!(matches!(result, Err(ColorError::ProfileParse(_))));
    }
}
```

### 6. Documentation Comments

**Every public item must have a `///` doc comment.** This is enforced by `#![warn(missing_docs)]` at the crate root. `cargo doc` must build with zero warnings.

#### What must be documented

- Every `pub struct`, `pub enum`, and `pub trait`
- Every `pub fn` and `pub method`
- Every `pub` field that is not self-evident from its name
- Every variant of a `pub enum`
- Every feature flag in `Cargo.toml` (documented in `lib.rs` module-level docs)

#### Doc comment structure

Follow this order for function/method docs:

```rust
/// Short one-line summary ending with a period.
///
/// Longer explanation if the behaviour is non-obvious. Describe *what* this
/// does, not *how* it does it. Include any invariants the caller must uphold.
///
/// # Arguments
///
/// Only include this section when argument semantics are not obvious from
/// the type and name alone. Do not list every argument mechanically.
///
/// # Errors
///
/// Document every `ColorError` variant this function can return and the
/// condition that causes it. This section is required on all functions
/// that return `ColorResult`.
///
/// # Examples
///
/// Every public function must have at least one `///` example that compiles
/// and passes as a doc-test.
///
/// ```rust
/// use appthere_color::{CmykColor, ColorValue};
///
/// let ink = CmykColor::new(0.0, 0.0, 0.0, 1.0); // pure black
/// assert_eq!(ink.key(), 1.0);
/// ```
pub fn example() { }
```

The `# Errors` section is **required** on every function returning `ColorResult<T>`. The `# Examples` section is **required** on every public function and method. Doc-tests are compiled and run as part of `cargo test` — they must pass.

#### Type and module docs

```rust
/// A complete document-level color management policy.
///
/// Captures the intended output condition, rendering intent, and fallback
/// behaviour when profiles are unavailable. Format-specific serialisation
/// (PDF `/OutputIntents`, ODF `draw:color-profile`) is the responsibility
/// of the consuming format crate.
///
/// # Examples
///
/// ```rust
/// use appthere_color::{ColorPolicy, IccProfile, RenderingIntent};
///
/// let policy = ColorPolicy::builder()
///     .default_rendering_intent(RenderingIntent::Perceptual)
///     .fallback_profile(IccProfile::srgb())
///     .build()?;
/// # Ok::<(), appthere_color::ColorError>(())
/// ```
pub struct ColorPolicy { /* ... */ }
```

Module-level `//!` comments are required in every `src/*.rs` file. They should state the module's single responsibility in one or two sentences.

#### Doc-test hygiene

- Use `# Ok::<(), appthere_color::ColorError>(())` to close fallible examples cleanly
- Use `# use appthere_color::...;` with a leading `#` to hide necessary imports that would clutter the example
- Do not use `# ignore` or `# no_run` unless the example genuinely cannot run in a test environment (e.g. it requires a real filesystem ICC file) — document why

#### Generating documentation

To build and open the full crate documentation locally:

```sh
cargo doc --no-deps --open
```

`--no-deps` omits dependency docs and keeps the output focused on `appthere-color` itself. To verify doc-tests pass:

```sh
cargo test --doc
```

To check for missing docs and broken intra-doc links without opening a browser:

```sh
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

This is the canonical documentation check command. It must pass with zero warnings before any PR is merged. CI runs this command on every push.

To include private items during development (useful when reviewing internal structure):

```sh
cargo doc --no-deps --document-private-items --open
```

### 7. API Design

- **Prefer builders for configuration types** — `ProofingConfig`, `ColorPolicy`, and `ColorTransform` should be constructed via a `Builder` pattern, not large constructors
- **Component ranges** — all color components are `f32` in the range `0.0..=1.0` unless the color space specifies otherwise (Lab: L is `0.0..=100.0`, a/b are `-128.0..=127.0`). Constructors clamp silently; transforms validate and return `Err` on out-of-range
- **Immutability** — color value types (`RgbColor`, `CmykColor`, etc.) are immutable after construction; transforms and policies use the builder pattern for configuration
- **Derive order** — always derive in this order: `Debug, Clone, PartialEq` (add `Copy` for small value types like `RenderingIntent`)

### 8. Dependencies

The dependency list is intentionally minimal. The permitted dependencies are:

| Crate | Purpose |
|---|---|
| `moxcms` | ICC profile loading and transform execution |
| `thiserror` | Error derive macros |

Do not add dependencies without a documented reason in the PR. In particular:
- Do not add `lcms2` or `lcms2-sys`
- Do not add `palette` (overlapping scope, LGPL-adjacent ecosystem)
- Do not add `image` (brings in format dependencies unrelated to color management)
- Do not add `serde` unless a concrete serialization need is identified — when added, gate it behind a `serde` feature flag

### 9. Cargo.toml Metadata

Keep the following fields populated and accurate at all times:

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

### 10. `no_std` Compatibility

The crate must remain `no_std` compatible. Guard any `std`-only code behind `#[cfg(feature = "std")]`. The default feature set includes `std`. Profile loading from filesystem paths is the only API that requires `std`.

Do not use `std::collections::HashMap` directly in `no_std`-compatible code — use `BTreeMap` or gate with `#[cfg(feature = "std")]`.

---

## Soft Proofing Implementation Notes

Soft proofing is implemented as **two chained transforms**:

1. Source profile → Output/simulation profile (using `simulation_intent`)
2. Output/simulation profile → Display profile (using `display_intent`, typically Relative Colorimetric)

This differs from `lcms2`'s native `Transform::new_proofing()` which handles this internally. The two-transform approach is semantically equivalent for standard workflows and avoids the `lcms2` dependency.

Gamut warning is applied at the boundary between transform 1 and transform 2: pixels that fall outside the output profile gamut are replaced or desaturated before the display transform runs.

---

## Color Policy Scope

`ColorPolicy` captures document-level color management intent. It maps directly to concepts found in:
- **PDF/X**: `/OutputIntents` array, `DestOutputProfile`
- **ODF**: `draw:color-profile` element in the document styles
- **EPUB**: ICC profile embedded in image resources

`appthere-color` represents these format-agnostically. The format-specific serialization (writing the ODF XML, embedding the ICC bytes in a PDF, etc.) is the responsibility of AppThere Loki's format crates. `ColorPolicy` provides the data; the format layer decides how to write it.

---

## Relationship to AppThere Loki

When working in the AppThere Loki workspace, `appthere-color` is consumed as a path dependency during development:

```toml
# In AppThere Loki's Cargo.toml during local development
appthere-color = { path = "../appthere-color" }

# In AppThere Loki's Cargo.toml for releases
appthere-color = "0.x"
```

Changes to the public API of `appthere-color` must be coordinated with updates to AppThere Loki's color service layer. Do not make breaking API changes without bumping the minor version and updating Loki simultaneously.

---

## What Good Looks Like

A well-formed contribution to this crate:
1. Adds or modifies exactly one logical concern per PR
2. Leaves no file over 300 lines
3. Contains zero `.unwrap()`, `.expect()`, or `unsafe`
4. Adds tests for every new public item, including doc-tests
5. Adds `///` doc comments with `# Errors` and `# Examples` on every new public item
6. `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes with zero warnings
7. Updates `CHANGELOG.md` with a summary entry
8. Does not add new dependencies without justification
