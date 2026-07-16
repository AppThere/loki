// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! VBA-stomping / tamper heuristic (macro spec §4.4, threat T5).
//!
//! "VBA stomping" wipes a module's *source* while leaving its compiled p-code
//! in place, so tools that read source see nothing while Office still executes
//! the hidden p-code. Loki never executes p-code, so a stomped module is inert
//! here — but we still **warn**, because a module that presents empty source
//! yet carries substantial compiled p-code is a strong tampering signal the
//! user should see before trusting the document.
//!
//! This is a best-effort heuristic (documented as such): it cannot be perfectly
//! precise without decoding p-code, which Loki deliberately refuses to do.

/// A per-module signal collected during extraction.
pub(crate) struct ModuleProbe {
    /// Whether the module's extracted source is empty/whitespace.
    pub(crate) source_empty: bool,
    /// Size of the compiled-cache region (bytes before the source text offset).
    pub(crate) pcode_size: usize,
}

/// A genuinely empty module still carries a small amount of p-code header; a
/// stomped one carries the compiled body of real code. This threshold
/// distinguishes "empty on purpose" from "source wiped".
const PCODE_SUSPICIOUS_BYTES: usize = 256;

/// Returns a tamper-warning message if any module looks stomped.
pub(crate) fn assess(probes: &[ModuleProbe]) -> Option<String> {
    let stomped = probes
        .iter()
        .filter(|p| p.source_empty && p.pcode_size > PCODE_SUSPICIOUS_BYTES)
        .count();
    if stomped == 0 {
        return None;
    }
    Some(format!(
        "This VBA project appears tampered: {stomped} module(s) have no readable \
         source but carry compiled code (possible \"VBA stomping\"). Loki never \
         runs compiled code, so nothing hidden can execute — but treat this \
         document with caution."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intact_project_has_no_warning() {
        let probes = vec![
            ModuleProbe {
                source_empty: false,
                pcode_size: 4000,
            },
            ModuleProbe {
                source_empty: true, // an intentionally empty module
                pcode_size: 20,
            },
        ];
        assert!(assess(&probes).is_none());
    }

    #[test]
    fn stomped_module_warns() {
        let probes = vec![ModuleProbe {
            source_empty: true,
            pcode_size: 5000,
        }];
        assert!(assess(&probes).is_some());
    }
}
