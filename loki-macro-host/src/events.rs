// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Auto-run event handlers (macro spec §5.6).
//!
//! Documents can bind code to lifecycle events — most dangerously *on open*
//! (`Document_Open`, `AutoOpen`, `Workbook_Open`, ODF `OnLoad`, …). Running such
//! code just because a document opened is the vector behind essentially all
//! macro malware (threat T1). This module only *recognises* the well-known
//! handler names; whether any of them may fire is decided elsewhere and gated
//! behind [`crate::MacroService::authorize_auto_run`] — nothing here runs
//! anything.

/// The lifecycle phase an event handler is bound to (spec §5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    /// Fires when the document opens — the T1 vector.
    Open,
    /// Fires when the document is about to close.
    Close,
    /// Fires when the document is saved.
    Save,
}

/// Well-known auto-run handler procedure names and the phase they fire in.
/// Case-insensitive by procedure name. VBA + `StarBasic`/sheet conventions.
const HANDLERS: &[(&str, EventPhase)] = &[
    // ── Open (T1) ───────────────────────────────────────────────────────────
    ("autoopen", EventPhase::Open),
    ("auto_open", EventPhase::Open),
    ("autoexec", EventPhase::Open),
    ("document_open", EventPhase::Open),
    ("workbook_open", EventPhase::Open),
    ("document_new", EventPhase::Open),
    ("workbook_activate", EventPhase::Open),
    ("onload", EventPhase::Open),
    ("onstartapp", EventPhase::Open),
    // ── Close ───────────────────────────────────────────────────────────────
    ("autoclose", EventPhase::Close),
    ("auto_close", EventPhase::Close),
    ("document_close", EventPhase::Close),
    ("workbook_beforeclose", EventPhase::Close),
    ("onunload", EventPhase::Close),
    // ── Save ────────────────────────────────────────────────────────────────
    ("document_beforesave", EventPhase::Save),
    ("workbook_beforesave", EventPhase::Save),
    ("onsave", EventPhase::Save),
];

/// The phase `name` is an auto-run handler for, or `None` if it is an ordinary
/// procedure (case-insensitive).
#[must_use]
pub fn handler_phase(name: &str) -> Option<EventPhase> {
    let lower = name.to_ascii_lowercase();
    HANDLERS
        .iter()
        .find(|(n, _)| *n == lower)
        .map(|(_, phase)| *phase)
}

/// Whether `name` is an on-**open** auto-run handler (the T1-relevant set).
#[must_use]
pub fn is_auto_open(name: &str) -> bool {
    handler_phase(name) == Some(EventPhase::Open)
}

/// Filters `proc_names` to the on-open auto-run handlers, preserving order.
/// Used by the app to decide what (if anything) may fire on open — but only
/// after [`crate::MacroService::authorize_auto_run`] has minted a token.
#[must_use]
pub fn auto_open_handlers<'a, I>(proc_names: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    proc_names
        .into_iter()
        .filter(|n| is_auto_open(n))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_open_handlers_case_insensitively() {
        for n in [
            "AutoOpen",
            "auto_open",
            "Document_Open",
            "Workbook_Open",
            "OnLoad",
        ] {
            assert_eq!(handler_phase(n), Some(EventPhase::Open), "{n}");
            assert!(is_auto_open(n));
        }
    }

    #[test]
    fn ordinary_procs_are_not_handlers() {
        for n in ["Main", "FormatTable", "Helper", "Sum"] {
            assert_eq!(handler_phase(n), None, "{n}");
            assert!(!is_auto_open(n));
        }
    }

    #[test]
    fn close_and_save_are_not_auto_open() {
        assert_eq!(handler_phase("AutoClose"), Some(EventPhase::Close));
        assert_eq!(handler_phase("Document_BeforeSave"), Some(EventPhase::Save));
        assert!(!is_auto_open("AutoClose"));
        assert!(!is_auto_open("Document_BeforeSave"));
    }

    #[test]
    fn filters_only_open_handlers() {
        let procs = ["Main", "Document_Open", "AutoClose", "AutoOpen"];
        assert_eq!(
            auto_open_handlers(procs),
            vec!["Document_Open".to_string(), "AutoOpen".to_string()]
        );
    }
}
