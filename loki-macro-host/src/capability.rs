// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The closed capability catalog and grant scopes (macro spec §5).
//!
//! Every effect a macro might request maps to exactly one [`Capability`]. The
//! set is a **closed enum**: adding a capability is a spec-level change, not a
//! patch (spec §5.2). Anything not in this catalog is refused outright by the
//! "never" list (spec §7) — there is no capability that enables process
//! spawning, FFI, COM/UNO, the registry, or path-addressed file I/O.

use serde::{Deserialize, Serialize};

/// An effect a macro can request, gated by the broker (spec §5.2).
///
/// Clipboard and file access are split into read/write halves because they are
/// separate consent decisions (reading the clipboard is an injection channel;
/// writing it is an exfiltration channel — the user grants each independently).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    /// Read the host document model, selection, and metadata. The baseline that
    /// makes macros useful; granted automatically once a document is enabled.
    DocRead,
    /// Mutate the host document via the object model (batched into one
    /// undo entry per run, spec §6.2).
    DocWrite,
    /// Show a `MsgBox`/`InputBox`/status message (badged + rate-limited, §5.5).
    UiDialog,
    /// Read the system clipboard.
    ClipboardRead,
    /// Write the system clipboard.
    ClipboardWrite,
    /// Read a file the user chose through the OS picker (picker == consent, and
    /// there is no path-addressed file API at all — spec §5.3).
    FileRead,
    /// Write to a picker-chosen target.
    FileWrite,
    /// Submit the document to the print flow.
    Print,
    /// Outbound network access. **Refused in v1** (spec §5.2, §14) — listed so
    /// the refusal is explicit and total, never a missing-grant prompt.
    Network,
}

impl Capability {
    /// Every capability, for exhaustive matrices and UI listing.
    pub const ALL: [Capability; 9] = [
        Capability::DocRead,
        Capability::DocWrite,
        Capability::UiDialog,
        Capability::ClipboardRead,
        Capability::ClipboardWrite,
        Capability::FileRead,
        Capability::FileWrite,
        Capability::Print,
        Capability::Network,
    ];

    /// A stable machine identifier used as the serialized key and the i18n
    /// lookup suffix (`macros-cap-<id>`). Never shown raw to the user.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Capability::DocRead => "doc-read",
            Capability::DocWrite => "doc-write",
            Capability::UiDialog => "ui-dialog",
            Capability::ClipboardRead => "clipboard-read",
            Capability::ClipboardWrite => "clipboard-write",
            Capability::FileRead => "file-read",
            Capability::FileWrite => "file-write",
            Capability::Print => "print",
            Capability::Network => "network",
        }
    }

    /// Whether this capability is permanently refused regardless of any grant
    /// (spec §5.2 / §7). Refusal is a property of the capability, not of the
    /// grant state, so it can never be prompted or persisted around.
    #[must_use]
    pub fn is_refused_in_v1(self) -> bool {
        matches!(self, Capability::Network)
    }

    /// Whether the capability is granted automatically once a document is
    /// enabled (spec §5.2: only `DocRead` is baseline). All others require a
    /// grant decision.
    #[must_use]
    pub fn is_baseline(self) -> bool {
        matches!(self, Capability::DocRead)
    }
}

/// How far a granted capability extends (spec §5.4).
///
/// There is deliberately **no "always for all documents"** scope — a grant
/// never escapes the document it was made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GrantScope {
    /// Refuse — the script sees a trappable error. The default prompt button.
    Deny,
    /// Allow for this run only (not remembered).
    AllowOnce,
    /// Allow until the document is closed (session memory, not persisted).
    AllowSession,
    /// Allow and persist to the document's trust record (spec §2.4); listed and
    /// revocable in the Document Security panel.
    AlwaysForDocument,
}

impl GrantScope {
    /// Whether this scope permits the effect (everything except [`Self::Deny`]).
    #[must_use]
    pub fn is_allow(self) -> bool {
        !matches!(self, GrantScope::Deny)
    }

    /// Whether a grant at this scope is written to the persistent trust record.
    #[must_use]
    pub fn is_persistent(self) -> bool {
        matches!(self, GrantScope::AlwaysForDocument)
    }
}

/// The broker's decision for a requested capability (spec §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityDecision {
    /// Proceed without asking — a baseline capability, or an existing allow
    /// grant in scope.
    Granted,
    /// Ask the user (the interpreter suspends at first use, spec §5.4). No
    /// grant is in scope yet.
    Prompt,
    /// Refuse with a **trappable** BASIC runtime error — an explicit deny grant,
    /// or a context (UDF) where prompting is impossible.
    Denied,
    /// Refuse with an **untrappable** `ErrFeatureRefused` — a "never" capability
    /// (spec §7). Distinct from [`Self::Denied`] so the host maps it to the
    /// feature-refused error the author sees.
    Refused,
}

/// The context a macro runs in, which changes the default decision (spec §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunContext {
    /// A user-invoked macro (Tools ▸ Macros ▸ Run, an assigned button). Prompts
    /// are possible; the full grant machinery applies.
    #[default]
    Interactive,
    /// A spreadsheet user-defined function evaluated during recalc. Runs with
    /// **zero** capabilities and cannot prompt: any effect request is
    /// [`CapabilityDecision::Denied`] (surfaced to the cell as `#MACRO!`).
    Udf,
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
