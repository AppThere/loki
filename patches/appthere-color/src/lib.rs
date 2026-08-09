//! Document-aware color management for Rust.
//!
//! `appthere-color` provides pure-Rust ICC color transforms, CMYK support,
//! soft proofing, and print-ready color policies. It is designed for document
//! authoring applications that need accurate color management without a C
//! toolchain dependency.
//!
//! # Features
//!
//! - **Color value types**: [`RgbColor`], [`CmykColor`], [`LabColor`],
//!   [`XyzColor`], [`GrayColor`], and [`ColorValue`]
//! - **ICC profile loading**: [`IccProfile`] wrapping `moxcms` for pure-Rust
//!   profile parsing
//! - **Color transforms**: [`ColorTransform`] builder for profile-to-profile
//!   conversion
//! - **Soft proofing**: [`ProofingConfig`] with chained two-stage transforms
//! - **Color policy**: [`ColorPolicy`] and [`OutputIntent`] for document-level
//!   color management
//!
//! # ICC Backend
//!
//! This crate uses [`moxcms`](https://crates.io/crates/moxcms) exclusively as
//! its ICC backend. No C toolchain is required — the crate cross-compiles
//! cleanly to Android, WASM, and musl targets.
//!
//! # Feature Flags
//!
//! - **`std`** (default): Enables file-path-based profile loading and I/O
//!   error support.
//! - **`serde`**: Enables `Serialize`/`Deserialize` derives on color types.
//!
//! # Example
//!
//! ```rust
//! use appthere_color::{
//!     ColorTransform, IccProfile, RenderingIntent, RgbColor,
//! };
//!
//! // Load profiles
//! let srgb = IccProfile::new_srgb();
//! let adobe = IccProfile::new_adobe_rgb();
//!
//! // Build a transform
//! let xform = ColorTransform::builder()
//!     .source(&srgb)
//!     .destination(&adobe)
//!     .intent(RenderingIntent::RelativeColorimetric)
//!     .build()
//!     .unwrap();
//!
//! // Transform a color
//! let input = [1.0_f32, 0.0, 0.0]; // pure red in sRGB
//! let mut output = [0.0_f32; 3];
//! xform.transform(&input, &mut output).unwrap();
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod color_space;
pub mod color_value;
pub mod color_value_ext;
pub mod error;
pub mod policy;
pub mod profile;
pub mod proofing;
pub mod proofing_builder;
pub mod rendering_intent;
pub mod transform;

// Re-export primary types at crate root for ergonomic access.
pub use color_space::ColorSpace;
pub use color_value::{CmykColor, LabColor, RgbColor};
pub use color_value_ext::{ColorValue, GrayColor, XyzColor};
pub use error::{ColorError, ColorResult};
pub use policy::{ColorPolicy, OutputIntent};
pub use profile::IccProfile;
pub use proofing::{GamutWarning, ProofingConfig};
pub use rendering_intent::RenderingIntent;
pub use transform::ColorTransform;
