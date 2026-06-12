// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Locale-based page-size helpers for [`super::Document::new_blank`].

/// Returns the appropriate default page size for the running locale.
///
/// Reads `LC_PAPER`, `LC_ALL`, `LANGUAGE`, and `LANG` environment variables
/// (in priority order).  Returns US Letter for regions that use it; A4 for
/// all others.  Falls back to A4 if no locale can be determined.
pub(super) fn default_page_size_for_locale() -> crate::layout::page::PageSize {
    use crate::layout::page::PageSize;
    for var in &["LC_PAPER", "LC_ALL", "LANGUAGE", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let upper = val.to_uppercase();
            if upper.is_empty() {
                continue;
            }
            if uses_letter_paper(&upper) {
                return PageSize::letter();
            }
            return PageSize::a4();
        }
    }
    PageSize::a4()
}

/// Returns `true` when `locale_upper` (an uppercased locale string) indicates
/// a region that uses US Letter paper by convention.
fn uses_letter_paper(locale_upper: &str) -> bool {
    const LETTER_REGIONS: &[&str] = &[
        "_US", "_CA", "_MX", "_PH", "_CO", "_CL", "_VE", "_BO", "_SV", "_GT", "_HN", "_NI",
        "_CR", "_DO", "_PR",
    ];
    LETTER_REGIONS.iter().any(|r| locale_upper.contains(r))
}
