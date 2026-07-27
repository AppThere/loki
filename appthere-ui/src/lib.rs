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
pub mod device_probe;
pub mod device_probe_memory;
pub mod device_profile;
pub mod device_profile_override;
pub mod responsive;
pub mod safe_area;
pub mod scroll;
pub mod theme;
pub mod tokens;

pub use components::icons::{
    AtIcon, AT_CHANGE_ACCEPT, AT_CHANGE_ACCEPT_ONE, AT_CHANGE_REJECT, AT_CHANGE_REJECT_ONE,
    AT_COLUMNS_ONE, AT_COLUMNS_THREE, AT_COLUMNS_TWO, AT_FONT_GROW, AT_FONT_SHRINK,
    AT_MARGIN_NARROW, AT_MARGIN_NORMAL, AT_MARGIN_WIDE, AT_PAGE_A4, AT_PAGE_LANDSCAPE,
    AT_PAGE_LETTER, AT_PAGE_PORTRAIT, AT_TABLE_COL_DELETE, AT_TABLE_COL_INSERT,
    AT_TABLE_COL_INSERT_LEFT, AT_TABLE_ROW_DELETE, AT_TABLE_ROW_INSERT, AT_TABLE_ROW_INSERT_ABOVE,
    AT_TOC_INSERT, AT_TOC_UPDATE, AT_TRACK_CHANGES, LUCIDE_ALIGN_CENTER, LUCIDE_ALIGN_JUSTIFY,
    LUCIDE_ALIGN_LEFT, LUCIDE_ALIGN_RIGHT, LUCIDE_BASELINE, LUCIDE_BOLD, LUCIDE_DOWNLOAD,
    LUCIDE_FOOTNOTE, LUCIDE_HIGHLIGHTER, LUCIDE_IMAGE, LUCIDE_ITALIC, LUCIDE_LAYOUT_TEMPLATE,
    LUCIDE_LINK, LUCIDE_MORE_HORIZONTAL, LUCIDE_PILCROW, LUCIDE_REDO, LUCIDE_SAVE,
    LUCIDE_STRIKETHROUGH, LUCIDE_SUBSCRIPT, LUCIDE_SUPERSCRIPT, LUCIDE_TABLE, LUCIDE_TRASH_2,
    LUCIDE_UNDERLINE, LUCIDE_UNDO,
};
pub use components::ribbon::{
    AtRibbon, AtRibbonGroup, AtRibbonGroups, AtRibbonIconButton, AtRibbonSelect, RibbonGroupSpec,
    RibbonTabDesc, RibbonTabIndex,
};
pub use components::{
    next_zoom, use_backdrop, use_provide_backdrop, AtBackdropContext, AtBackdropHost,
    AtColorPickerLabels, AtColorPickerPanel, AtColorPickerTrigger, AtColorSwatch, AtConfirmDialog,
    AtConfirmDialogProps, AtDocumentTab, AtDocumentTabData, AtDocumentTabProps, AtHomeTab,
    AtHomeTabProps, AtInfobar, AtInfobarProps, AtMacroTrustDialog, AtMacroTrustDialogProps,
    AtNetworkPrompt, AtNetworkPromptProps, AtPanelHost, AtPanelHostProps, AtPermissionPrompt,
    AtPermissionPromptProps, AtStatusBar, AtStatusBarProps, AtTabBar, AtTabBarProps,
    AtTemplateBrowser, AtTemplateBrowserProps, AtTitleBar, AtTitleBarProps, BuiltinTemplate,
    MacroDialogFrame, MacroDialogFrameProps, MacroGrantChoice, MacroTrustChoice, PanelPosture,
    Platform, RecentDocument, BACKDROP_Z_INDEX,
};
pub use device_probe::{
    note_device_scale_factor, note_gpu_class, note_system_memory, parse_meminfo,
    probe_system_memory, SystemMemory,
};
pub use device_probe_memory::{
    quantise_bytes, quantised, use_memory_resampling, MEMORY_QUANTUM_BYTES, MEMORY_RESAMPLE_SECS,
};
pub use device_profile::{
    note_pointer, use_device_profile, use_provide_device_profile, AtDeviceProfileContext,
    DeviceProfile, GpuClass, PhysicalDisplay, PointerPrecision, WindowMode,
};
pub use device_profile_override::{
    current as device_profile_override, describe as describe_device_profile_override,
    ProfileOverride, OVERRIDE_ENV as DEVICE_PROFILE_OVERRIDE_ENV,
};
pub use responsive::{
    estimate_group_metrics, group_layout, page_fits, required_page_width, resolve_cascade,
    resolve_page_fit, use_breakpoint, use_provide_responsive, use_responsive, use_ribbon_cascade,
    use_viewport, AtResponsiveContext, AtViewportWidthSensor, AtWindowSizeSensor, Breakpoint,
    GroupCollapse, GroupLayout, GroupMetrics, PageFit, RibbonCascade, Viewport, DEFAULT_DPI,
};
pub use safe_area::{set_safe_area_insets, update_safe_area_insets, use_safe_area, SafeAreaInsets};
pub use scroll::{
    use_viewport_controller, ContentRect, MotionPreference, RevealMargin, ScrollMetrics,
    ViewportController,
};
pub use theme::{use_theme, AtThemeContext, ThemeVariant};
