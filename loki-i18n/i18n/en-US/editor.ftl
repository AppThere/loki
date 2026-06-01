# Document editor chrome.

# Status bar
editor-page-label = Page { $current } of { $total }
# Intentionally empty — shows nothing while the document is loading.
editor-page-loading =
editor-word-count = { $count ->
    [one]   1 word
   *[other] { $count } words
}
editor-language = English (US)
editor-zoom-aria = Zoom level

# Document title for unsaved blank documents
editor-untitled = Untitled
editor-untitled-n = Untitled { $n }

# Status bar app-specific labels
editor-sheet-label = Sheet { $current } of { $total }
editor-slide-label = Slide { $current } of { $total }

# Font warning banner
editor-font-substitution-title = Font Substitution
editor-font-substitution-message = Some fonts in this document are not available and were substituted. Layout and formatting might differ from Office 365.
editor-font-substitution-download = Download original fonts:
editor-font-dismiss = Dismiss

# Save
editor-save-success = Document saved
editor-save-error = Could not save: { $reason }
editor-save-untitled-hint = Use File → Save As to save new documents
