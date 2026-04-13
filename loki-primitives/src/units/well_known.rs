// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

use super::{unit_types::*, length::Length};

/// Generic points layout
pub type Points = Length<Pt>;
/// Pixels layout definition
pub type Pixels = Length<Px>;
/// Millimeters type alias
pub type Millimeters = Length<Mm>;
/// Inches type alias
pub type Inches = Length<Inch>;
/// English metric unit measurement
pub type Emus = Length<Emu>;
/// Typographical TWIPs alias
pub type Twips = Length<Twip>;
