// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
