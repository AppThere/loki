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
