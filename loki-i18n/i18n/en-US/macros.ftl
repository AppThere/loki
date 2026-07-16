# Macro security — the "document contains macros" notice and trust choices.
# Loki disables macros by default for documents the user did not author
# (macro spec §2, §9). Phase 1 has no execution; these strings cover the
# passive "macros are present and disabled" surface.

# Infobar shown under the ribbon when an opened document carries macros.
macros-infobar-disabled = This document contains macros. Macros are disabled.
macros-infobar-action = Enable options…
macros-infobar-dismiss = Dismiss macro notice

macros-view-action = View macros…

# Status-bar chip shown after the infobar is dismissed (or on later opens).
macros-status-chip = Macros disabled

# Read-only macro source viewer (Phase 3 — visibility before executability).
macros-viewer-title = Macros (read-only)
macros-viewer-close = Close
macros-viewer-empty = No readable macro source was found in this document.
macros-viewer-unreadable = The macro project could not be read. It is preserved in the file but cannot be shown.
macros-viewer-tamper =
    This project appears tampered: some modules have no readable source but
    carry compiled code (possible "VBA stomping"). Loki never runs compiled
    code, so nothing hidden can execute — but treat this document with caution.

# Trust dialog — the three choices from macro spec §2.3.
macros-trust-title = Enable macros for this document?
macros-trust-message =
    This document contains macros. Enable them only if you trust the source.
    Macros can change the document and access features you have granted.
macros-trust-keep-disabled = Keep disabled
macros-trust-session = Enable for this session
macros-trust-always = Trust this document
