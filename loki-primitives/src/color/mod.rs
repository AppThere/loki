// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document-semantic color and theme palettes.

mod document;
mod theme;

pub use document::{ColorParseError, DocumentColor};
pub use theme::{ThemeColor, ThemeColorSlot};

/// A print-ready CMYK color from `appthere_color`. See [`appthere_color::CmykColor`].
pub use appthere_color::CmykColor;

/// A library error from `appthere_color`. See [`appthere_color::ColorError`].
pub use appthere_color::ColorError;

/// A generic color result from `appthere_color`. See [`appthere_color::ColorResult`].
pub use appthere_color::ColorResult;

/// A color space profile wrapper from `appthere_color`. See [`appthere_color::ColorSpace`].
pub use appthere_color::ColorSpace;

/// Value types abstracting color encoding from `appthere_color`. See [`appthere_color::ColorValue`].
pub use appthere_color::ColorValue;

/// An uncalibrated gray/intensity from `appthere_color`. See [`appthere_color::GrayColor`].
pub use appthere_color::GrayColor;

/// A CIE L*a*b* uniform color mapping from `appthere_color`. See [`appthere_color::LabColor`].
pub use appthere_color::LabColor;

/// A primary RGB triple from `appthere_color`. See [`appthere_color::RgbColor`].
pub use appthere_color::RgbColor;

/// A CIE XYZ reference color matching space coordinate from `appthere_color`. See [`appthere_color::XyzColor`].
pub use appthere_color::XyzColor;
