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

# ── Phase 4: capability prompts, security panel, anti-spoof frame ─────────────

# Badge word shown in the reserved-accent frame of every macro-originated dialog
# (macro spec §5.5). Never used by app chrome.
macros-badge = Macro

# First-use capability prompt buttons (macro spec §5.4). Deny is the default.
macros-perm-deny = Deny
macros-perm-allow-once = Allow once
macros-perm-allow-session = Allow for this session
macros-perm-allow-always = Always for this document

# Capability names + plain-language consequences. Keys are
# `macros-cap-{id}-name/-title/-consequence` where {id} matches Capability::id().
macros-cap-doc-read-name = Read this document
macros-cap-doc-write-name = Change this document
macros-cap-doc-write-title = Change this document?
macros-cap-doc-write-consequence =
    The macro wants to modify the document's contents. Changes are one undo step.
macros-cap-ui-dialog-name = Show dialogs
macros-cap-ui-dialog-title = Show a message?
macros-cap-ui-dialog-consequence =
    The macro wants to display a message or ask for input.
macros-cap-clipboard-read-name = Read the clipboard
macros-cap-clipboard-read-title = Read the clipboard?
macros-cap-clipboard-read-consequence =
    The macro wants to read whatever you last copied.
macros-cap-clipboard-write-name = Write the clipboard
macros-cap-clipboard-write-title = Write the clipboard?
macros-cap-clipboard-write-consequence =
    The macro wants to replace the clipboard contents.
macros-cap-file-read-name = Read a file
macros-cap-file-read-title = Read a file?
macros-cap-file-read-consequence =
    The macro wants to read a file you choose from the file picker.
macros-cap-file-write-name = Write a file
macros-cap-file-write-title = Write a file?
macros-cap-file-write-consequence =
    The macro wants to write to a file you choose from the file picker.
macros-cap-print-name = Print
macros-cap-print-title = Print this document?
macros-cap-print-consequence = The macro wants to send the document to the printer.
macros-cap-network-name = Network access

# Document Security panel (macro spec §9.4).
macros-security-title = Document security
macros-security-close = Close
macros-security-open-action = Document security…
macros-security-state-label = Macro trust
macros-security-state-disabled = Macros disabled
macros-security-state-session = Enabled for this session
macros-security-state-trusted = Trusted
macros-security-enable = Enable options…
macros-security-auto-run = Run this document's macros automatically when it opens
macros-security-auto-run-hint = Recommended only for documents you created.
macros-security-capabilities = Granted capabilities
macros-security-cap-none = No capabilities have been granted.
macros-security-cap-session-tag = this session
macros-security-cap-always-tag = always
macros-security-cap-refused = Refused
macros-security-cap-revoke = Revoke
macros-security-forget = Forget this document
macros-security-manage-title = Trusted documents
macros-security-manage-empty = No documents are trusted on this device.
macros-security-manage-forget = Forget
