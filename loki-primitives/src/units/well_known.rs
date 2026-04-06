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
