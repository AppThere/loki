// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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

    /// Whether an element wider than the content column is scaled down to fit
    /// it (Spec 08 T7.3), rather than being allowed to overhang.
    ///
    /// True only for [`Self::Reflow`]. Reflow is a *reading* view with no
    /// physical width, and T7.3's rule is that the document never scrolls
    /// horizontally, so anything oversized must come down to the column.
    ///
    /// [`Self::Paginated`] and [`Self::Pageless`] are **fidelity** views of a
    /// page with a real width: Word and LibreOffice paint an oversized image at
    /// its declared size and let it overhang, and matching them is the point.
    ///
    /// A method on the mode rather than a flag each caller derives — the two
    /// call sites that need it are in different modules, and a second
    /// `matches!(mode, Reflow)` written by hand is the copy that stops agreeing.
    #[must_use]
    pub fn fits_oversized_to_column(&self) -> bool {
        matches!(self, Self::Reflow { .. })
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
        let m = LayoutMode::Reflow {
            available_width: 400.0,
        };
        assert!(!m.is_paginated());
        assert!(m.is_continuous());
    }
}
