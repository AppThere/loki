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
# Shown while the first word count is still being computed. Deliberately not
# "0 words": a figure nobody has computed yet must not read as a figure (I-10).
editor-word-count-pending = Counting words…
editor-language = English (US)
editor-zoom-aria = Zoom level
editor-zoom-out = Zoom out
editor-zoom-in = Zoom in
editor-zoom-menu = Zoom level, opens preset list
editor-zoom-fit-width = Fit width
editor-zoom-fit-page = Fit page
editor-zoom-actual-size = Actual size
editor-zoom-reduced = Reduced to fit this device's memory

# Display calibration for Actual Size — shown on first use, never at first run.
editor-calibrate-title = Measure your screen
editor-calibrate-instructions = Hold a bank card or a ruler against the blue bar. It should be { $mm } mm wide — type what it really measures.
editor-calibrate-field = Measured width (mm)
editor-calibrate-apply = Use this
editor-calibrate-cancel = Cancel
editor-color-area = Saturation and brightness
editor-color-hue = Hue

editor-calibrate-rejected = That doesn't look like a measurement of this bar. Check you're using millimetres and try again.

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

# Font substitution — status-bar chip (the indicator) + detail panel
editor-font-substitution-title = Font Substitution
editor-font-substitution-message = Some fonts in this document are not available and were substituted. Layout and formatting might differ from Office 365.
# Status-bar indicator chip; clicking it opens the detail panel
editor-font-substitution-chip = { $count ->
    [one] 1 font substituted
   *[other] { $count } fonts substituted
}
editor-font-substitution-close = Close
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
# Suggested filename for Save a Copy, from the current document's stem.
editor-save-copy-suggested-name = { $stem } (copy).docx
editor-save-template-success = Template saved
editor-save-error = Could not save: { $reason }
editor-save-untitled-hint = Use File → Save As to save new documents
editor-dismiss-aria = Dismiss

# Word-compatibility repair banner (shown when an opened DOCX has out-of-order
# OOXML that can stop Microsoft Word opening it; Loki opens it regardless)
editor-repair-title = Compatibility problem
editor-repair-message = { $count ->
    [one] This document has 1 issue that can stop it opening in Microsoft Word.
   *[other] This document has { $count } issues that can stop it opening in Microsoft Word.
}
editor-repair-action = Repair
editor-repair-dismiss = Dismiss
editor-repair-done = { $count ->
    [one] Repaired 1 compatibility issue
   *[other] Repaired { $count } compatibility issues
}
editor-repair-error = Could not repair: { $reason }

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

# Insert → Table / Footnote (Spec 04 M4) — status banner messages
editor-insert-table-success = Table inserted
editor-insert-footnote-success = Footnote inserted
editor-insert-no-cursor = Place the cursor in the document first
editor-insert-failed = Could not insert here

# Reflow view → oversized elements (Spec 08 T7.3). A block wider than the
# reading column gets its own horizontal scroll container; the toggle chooses
# between fitting it to the column and letting it scroll at its own width.
editor-oversized-expand = Expand
editor-oversized-fit = Fit to column
editor-insert-section-ok = Section inserted

# §6 — the Print dialog
print-dialog-title = Print
print-dialog-close-aria = Close print dialog
print-dialog-cancel = Cancel
print-system-hint = Print through your system's print dialog — choose the printer, pages, and copies there.
print-system-button = Print…
print-ipp-hint = Or send directly to a network printer (IPP).
print-ipp-uri-label = Printer URI
print-ipp-copies = Copies
print-ipp-pages = Pages
print-ipp-duplex = Print on both sides
print-ipp-button = Send to printer
print-ipp-sent = Job { $job } sent to printer
print-sent = Sent to the system print service
print-cancelled = Printing cancelled
print-error = Could not print: { $reason }
editor-page-break-nested = Page breaks only apply to top-level paragraphs
