// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The "never" list (macro spec §7): language surface that is refused
//! **unconditionally** — no capability, prompt, or configuration can enable it.
//!
//! These are refused in the interpreter itself, not the host, because the
//! refusal is a pure property of the language (it needs no ambient authority to
//! decide). A call to any of these names raises an **untrappable**
//! [`crate::RuntimeError::feature_refused`] so a malicious macro cannot
//! `On Error Resume Next` past it (spec §7, threat T2/T3). Preserved payloads
//! may *contain* these; they simply never execute.
//!
//! FFI (`Declare … Lib`) is refused separately, by name, in the interpreter
//! (the declared name is tracked at index time), since the name is arbitrary.

/// The refused identifiers, lowercased. Grouped by the §7 table row they close.
const REFUSED: &[&str] = &[
    // Process execution — the dropper endgame (T2).
    "shell",
    "shellexecute",
    "sendkeys",
    "appactivate",
    "exec",
    // COM / OLE automation — unbounded external surface (T2, T4).
    "createobject",
    "getobject",
    // UNO service manager — the StarBasic flavour of the same.
    "createunoservice",
    "createunolistener",
    "createunostruct",
    "getprocessservicemanager",
    "getdefaultcontext",
    // Path-addressed file I/O — replaced by picker-mediated handles (T3, §5.3).
    "kill",
    "filecopy",
    "mkdir",
    "rmdir",
    "chdir",
    "chdrive",
    "setattr",
    "getattr",
    "dir",
    "curdir",
    "freefile",
    "filelen",
    "filedatetime",
    // Registry / OS settings — persistence & recon.
    "getsetting",
    "savesetting",
    "deletesetting",
    "getallsettings",
    "environ",
    // DDE — legacy exec vector.
    "ddeinitiate",
    "ddeexecute",
    "dderequest",
    "ddepoke",
    "ddeterminate",
    "ddeterminateall",
    // Timer-based background execution — macros run only in a visible,
    // cancellable session (§8).
    "ontime",
    "settimer",
    "timer",
];

/// Whether `name` is a refused "never"-list identifier (case-insensitive).
#[must_use]
pub(super) fn is_refused(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    REFUSED.contains(&lower.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_known_dangerous_names_case_insensitively() {
        for n in [
            "Shell",
            "SHELL",
            "CreateObject",
            "Kill",
            "Environ",
            "createunoservice",
        ] {
            assert!(is_refused(n), "{n} should be refused");
        }
    }

    #[test]
    fn does_not_refuse_ordinary_names() {
        for n in ["Foo", "MsgBox", "Left", "ActiveDocument", "x"] {
            assert!(!is_refused(n), "{n} should not be refused");
        }
    }
}
