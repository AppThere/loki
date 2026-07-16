# Macro security — the "document contains macros" notice and trust choices.
# Loki disables macros by default for documents the user did not author
# (macro spec §2, §9). Phase 1 has no execution; these strings cover the
# passive "macros are present and disabled" surface.

# Infobar shown under the ribbon when an opened document carries macros.
macros-infobar-disabled = This document contains macros. Macros are disabled.
macros-infobar-action = Enable options…
macros-infobar-dismiss = Dismiss macro notice

# Status-bar chip shown after the infobar is dismissed (or on later opens).
macros-status-chip = Macros disabled

# Trust dialog — the three choices from macro spec §2.3.
macros-trust-title = Enable macros for this document?
macros-trust-message =
    This document contains macros. Enable them only if you trust the source.
    Macros can change the document and access features you have granted.
macros-trust-keep-disabled = Keep disabled
macros-trust-session = Enable for this session
macros-trust-always = Trust this document
