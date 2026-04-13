// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Layout mode definitions.
//!
//! The [`LayoutMode`] enum controls how the layout engine distributes content:
//! onto fixed pages, or onto a single infinite canvas.

/// The three layout modes.
///
/// `Reflow` and `Pageless` use the same algorithm; they differ only in where
/// the content width comes from.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutMode {
    /// Fixed pages. Content is broken into pages matching the document's
    /// `PageLayout` dimensions. Respects headers, footers, widow/orphan rules.
    Paginated,

    /// Single infinite canvas. Width = document page width minus margins.
    /// No page breaks, no headers/footers.
    Pageless,

    /// Single infinite canvas. Width = caller-supplied container width.
    ///
    /// Used when the container is narrower than the document page width
    /// (mobile, small windows). Same algorithm as `Pageless` with the
    /// content width overridden.
    Reflow {
        /// Available container width in points.
        available_width: f32,
    },
}

impl LayoutMode {
    /// Returns `true` if this mode produces pages (paginated layout).
    pub fn is_paginated(&self) -> bool {
        matches!(self, Self::Paginated)
    }

    /// Returns `true` if this mode produces a single continuous canvas.
    pub fn is_continuous(&self) -> bool {
        !self.is_paginated()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paginated_flags() {
        let m = LayoutMode::Paginated;
        assert!(m.is_paginated());
        assert!(!m.is_continuous());
    }

    #[test]
    fn pageless_flags() {
        let m = LayoutMode::Pageless;
        assert!(!m.is_paginated());
        assert!(m.is_continuous());
    }

    #[test]
    fn reflow_flags() {
        let m = LayoutMode::Reflow { available_width: 400.0 };
        assert!(!m.is_paginated());
        assert!(m.is_continuous());
    }
}
