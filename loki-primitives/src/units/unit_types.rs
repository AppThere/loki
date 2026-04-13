// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

/// PostScript points. 1 pt = 1/72 inch. The base typographic unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pt;

/// Screen pixels. Device-dependent. Cannot be converted to physical units
/// without a DPI value — see ADR-0003.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Px;

/// Millimeters. Used in ODF measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mm;

/// Inches. Used in OOXML measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Inch;

/// English Metric Units. 914400 EMU = 1 inch. Used in OOXML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Emu;

/// Twips. 1440 twips = 1 inch. Used in legacy OOXML (Word) measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Twip;
