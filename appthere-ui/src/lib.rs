// SPDX-License-Identifier: Apache-2.0

//! AppThere suite shared UI components and design tokens (`appthere_ui`).
//!
//! # Structure
//!
//! * [`tokens`] — design-token constants (colors, spacing, typography, layout).
//!   Import via `use appthere_ui::tokens::*` or name individual sub-modules.
//! * [`theme`] — [`AtThemeContext`] and [`use_theme`] for injecting the active
//!   theme variant from the app root to all descendant components.
//! * [`components`] — shell component primitives shared across all AppThere
//!   suite applications (title bar, tab bar, home tab, status bar).
//!
//! # Usage
//!
//! Inject the theme context at the application root:
//! ```rust,ignore
//! provide_context(AtThemeContext::default()); // ThemeVariant::Dark
//! ```
//! Then use any shell component:
//! ```rust,ignore
//! AtStatusBar { page_label: "Page 1 of 1", .. }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod components;
pub mod responsive;
pub mod safe_area;
pub mod theme;
pub mod tokens;

pub use components::icons::{
    AtIcon, LUCIDE_ALIGN_CENTER, LUCIDE_ALIGN_JUSTIFY, LUCIDE_ALIGN_LEFT, LUCIDE_ALIGN_RIGHT,
    LUCIDE_BOLD, LUCIDE_DOWNLOAD, LUCIDE_FOOTNOTE, LUCIDE_IMAGE, LUCIDE_ITALIC,
    LUCIDE_LAYOUT_TEMPLATE, LUCIDE_LINK, LUCIDE_PILCROW, LUCIDE_REDO, LUCIDE_SAVE,
    LUCIDE_STRIKETHROUGH, LUCIDE_SUBSCRIPT, LUCIDE_SUPERSCRIPT, LUCIDE_TABLE, LUCIDE_TRASH_2,
    LUCIDE_UNDERLINE, LUCIDE_UNDO,
};
pub use components::ribbon::{
    AtRibbon, AtRibbonGroup, AtRibbonIconButton, AtRibbonSelect, RibbonTabDesc, RibbonTabIndex,
};
pub use components::{
    next_zoom, AtConfirmDialog, AtConfirmDialogProps, AtDocumentTab, AtDocumentTabData,
    AtDocumentTabProps, AtHomeTab, AtHomeTabProps, AtPanelHost, AtPanelHostProps, AtStatusBar,
    AtStatusBarProps, AtTabBar, AtTabBarProps, AtTitleBar, AtTitleBarProps, BuiltinTemplate,
    PanelPosture, Platform, RecentDocument,
};
pub use responsive::{
    page_fits, required_page_width, resolve_page_fit, use_breakpoint, use_provide_responsive,
    use_responsive, use_viewport, AtResponsiveContext, Breakpoint, PageFit, Viewport, DEFAULT_DPI,
};
pub use safe_area::{set_safe_area_insets, update_safe_area_insets, use_safe_area, SafeAreaInsets};
pub use theme::{use_theme, AtThemeContext, ThemeVariant};
