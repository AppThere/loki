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

# Macro runner (Tools ▸ Macros, Phase 5). Runs on a worker thread; capability
# prompts and dialogs render live and Stop is always available.
macros-run-open-action = Run a macro…
macros-run-title = Run a macro
macros-run-close = Close
macros-run-action = Run
macros-run-stop = Stop
macros-dialog-ok = OK
macros-dialog-cancel = Cancel
macros-run-none = No runnable procedures were found in this document.
macros-run-done = Macro finished.
macros-run-done-edited = Macro finished — the document was changed (undo with ⌘Z / Ctrl+Z).
macros-run-refused =
    The macro tried to use a feature Loki never allows (for example running a
    program or reading a file by path). It was stopped.
macros-run-denied =
    The macro needed a permission that was denied, so it stopped. Run it again
    to allow the permission if you trust it.
macros-run-stopped = The macro was stopped: it ran too long or was cancelled.
macros-run-unreadable = The macro could not be run.

# Macro editor (edit + source-only write-back, spec §3.4)
macros-edit-open-action = Edit macros…
macros-editor-title = Edit macros
macros-editor-save = Save
macros-editor-close = Close
macros-editor-hint =
    Saves source only — Word or LibreOffice recompiles the macros when the file
    is reopened.
macros-editor-saved = Macros updated — saving the document.
macros-editor-failed = The macros couldn't be updated, so they were left unchanged.

# ── Phase 8A: digital signatures & trusted publishers (ADR-0014 §5) ───────────

# Signature state shown in the Document Security panel. { $name } is the signer
# common name; { $issuer } the issuing authority; { $thumbprint } the hex
# certificate thumbprint.
macros-sig-label = Signature
macros-sig-unsigned = Not digitally signed
macros-sig-invalid = Signature invalid — treated as unsigned
macros-sig-trusted = Signed by { $name } (trusted publisher)
macros-sig-untrusted = Signed by { $name } — not trusted
macros-sig-legacy = Signed by { $name } — with a method Loki will not trust
macros-sig-expired = Signed by { $name } — the certificate has expired
macros-sig-renewed = { $name } renewed their certificate
macros-sig-issuer = Issued by { $issuer }
macros-sig-thumbprint = Thumbprint { $thumbprint }
macros-sig-trusted-note =
    Macros from this publisher are enabled when the document opens. Every
    sensitive action still asks first.

# Infobar line for a document enabled because its signer is a trusted publisher.
macros-sig-enabled-publisher = Macros enabled — signed by a trusted publisher.

# Warning shown atop the macro editor for a signed project (ADR-0014 §4.6):
# editing invalidates the signature.
macros-sig-edit-warning =
    Editing these macros removes the publisher's digital signature. The document
    will fall back to per-document trust.

# "Trust this publisher" affordance and its confirmation (anti-spoof frame).
macros-sig-trust-action = Trust this publisher…
macros-sig-trust-renewed-action = Trust the renewed certificate…
macros-sig-trust-title = Trust this publisher?
macros-sig-trust-message =
    Loki will enable macros signed by { $name } automatically whenever you open
    a document. Only trust a publisher you recognise — this is exactly what an
    attacker would try to impersonate.
macros-sig-trust-confirm = Trust publisher
macros-sig-trust-cancel = Cancel

# Trusted-publisher management list (Document Security panel).
macros-sig-manage-title = Trusted publishers
macros-sig-manage-empty = No publishers are trusted on this device.
macros-sig-manage-remove = Remove
macros-sig-manage-revocation-note =
    Removing a publisher is the only way to distrust it — Loki does not check
    online certificate revocation.
