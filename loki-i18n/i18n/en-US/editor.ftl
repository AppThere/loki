# Document editor chrome.

# Status bar
editor-page-label = Page { $current } of { $total }
# Intentionally empty — shows nothing while the document is loading.
editor-page-loading =
# Shown on the blank page placeholder while a document opens.
editor-document-loading = Opening document…
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

# Presentation preview banner — shown because .pptx/.odp loading and saving
# are not yet implemented; the slides on screen are a non-editable sample.
editor-presentation-preview = Preview only — opening and saving presentations isn't supported yet. These sample slides are not your file, and changes won't be saved.
# Shown when a real .pptx is opened (read-only import).
editor-presentation-readonly = Read-only preview — editing and saving presentations is coming soon.
# Shown when an opened presentation has no slides.
editor-presentation-empty = This presentation has no slides.
# Shown when a presentation file fails to open.
editor-load-failed = Could not open this presentation: { $reason }
# Shown while a presentation is loading.
editor-presentation-loading = Loading…
# Presentation editor toolbar actions.
editor-action-save = Save
editor-action-add-slide = Add slide
editor-action-delete-slide = Delete slide
editor-action-add-bullet = Add bullet
# Placeholder prompts for empty presentation text fields.
editor-placeholder-title = Title
editor-placeholder-subtitle = Subtitle

# Font warning banner
editor-font-substitution-title = Font Substitution
editor-font-substitution-message = Some fonts in this document are not available and were substituted. Layout and formatting might differ from Office 365.
editor-font-substitution-download = Download original fonts:
editor-font-dismiss = Dismiss
# Compact chip + recovery indicator (one concise line)
editor-font-substitution-chip = { $count ->
    [one] 1 font substituted
   *[other] { $count } fonts substituted
}
editor-font-substitution-details = Details
editor-font-substitution-collapse = Hide
# Per-substitution card / row
editor-font-substitution-requested = Requested
editor-font-substitution-substitute = Substitute
editor-font-substitution-missing = Missing — no substitute
# Severity
editor-font-substitution-compatible = Metric-compatible
editor-font-substitution-approx = Layout may shift
editor-font-download-original = Download original

# Save
editor-save-success = Document saved
editor-save-template-success = Template saved
editor-save-error = Could not save: { $reason }
editor-save-untitled-hint = Use File → Save As to save new documents
editor-dismiss-aria = Dismiss

# Style picker / style editor chrome
editor-tab-close-aria = Close tab
editor-style-picker-close-aria = Close style picker
editor-style-editor-close-aria = Close style editor
editor-style-based-on-label = Based on
editor-style-next-style-label = Next style
editor-style-align-label = Align
editor-style-name-label = Name
editor-style-new = + New
editor-style-font-label = Font
editor-style-size-label = Size (pt)
editor-style-weight-label = Weight
editor-style-weight-thin = Thin
editor-style-weight-light = Light
editor-style-weight-regular = Regular
editor-style-weight-medium = Medium
editor-style-weight-semibold = Semibold
editor-style-weight-bold = Bold
editor-style-weight-black = Black
editor-style-align-left = Left
editor-style-align-center = Center
editor-style-align-right = Right
editor-style-align-justify = Justify
editor-style-indent-label = Indent
editor-style-indent-left = Left
editor-style-indent-right = Right
editor-style-indent-first = First line
editor-style-indent-hanging = Hanging
editor-style-spacing-label = Spacing
editor-style-spacing-before = Before
editor-style-spacing-after = After
editor-style-line-spacing = Line ×

# View mode (paginated vs reflowed) toggle in the status bar
editor-view-paginated = Paginated
editor-view-reflowed = Reflowed
editor-view-toggle-aria = Toggle between paginated and reflowed view

# Spelling suggestions panel (right-click on a word) and language picker
editor-spelling-heading = Spelling: { $word }
editor-spelling-correct = { $word } — correctly spelled
editor-spelling-no-suggestions = No suggestions
editor-spelling-add-dictionary = Add to Dictionary
editor-spelling-ignore = Ignore
editor-spelling-language = Spelling Language…
editor-spelling-language-title = Spelling Language
editor-spelling-license = License: { $license }
editor-spelling-active = Active
editor-spelling-use = Use
editor-spelling-download = Download
editor-spelling-downloading = Downloading…
editor-spelling-download-ok = Downloaded and activated
editor-spelling-download-failed = Download failed
editor-spelling-load-failed = Could not load dictionary

# Insert → Image (Spec 04 M4) — status banner messages
editor-insert-image-success = Image inserted
editor-insert-image-unsupported = Unsupported image format
editor-insert-image-no-cursor = Place the cursor in the text, then insert the image
editor-insert-image-error = Could not insert image: { $reason }
