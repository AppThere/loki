# Ribbon tabs and controls.

# Tab labels
# The ribbon's primary tab is "Write" (renamed from "Home" — Spec 04 D1 — to
# resolve the collision with the application Home *screen*).
ribbon-tab-write = Write
# Span-level (character) formatting: font colour and highlight pickers.
ribbon-tab-format = Format

# Inline formatting group (Write tab)
ribbon-group-inline = Inline formatting
ribbon-bold-aria = Bold (Ctrl+B)
ribbon-italic-aria = Italic (Ctrl+I)
ribbon-underline-aria = Underline (Ctrl+U)
ribbon-strikethrough-aria = Strikethrough
ribbon-superscript-aria = Superscript
ribbon-subscript-aria = Subscript

# History group (Home tab)
ribbon-group-history = History
ribbon-undo-aria = Undo (Ctrl+Z)
ribbon-redo-aria = Redo (Ctrl+Y)

# Document group (Home tab)
ribbon-group-document = Document
# The three split buttons: each pairs a primary action with an anchored menu.
ribbon-new-aria = New document (Ctrl+N)
ribbon-new-menu-aria = New from template…
ribbon-open-aria = Open… (Ctrl+O)
ribbon-open-menu-aria = Open recent…
ribbon-open-no-recents = No recent documents
ribbon-save-aria = Save (Ctrl+S)
ribbon-save-menu-aria = More save options…
ribbon-save-as-label = Save As…
ribbon-save-a-copy-label = Save a Copy…
ribbon-save-as-template-label = Save as Template…

# Lists group (Write tab)
ribbon-group-lists = Lists
ribbon-inline-more-aria = More text formatting
ribbon-group-page-insert = Page
ribbon-insert-section-break-aria = Insert section break
ribbon-group-breaks = Breaks
ribbon-insert-page-break-aria = Insert page break
ribbon-insert-para-break-aria = Insert paragraph break
ribbon-insert-line-break-aria = Insert line break
ribbon-insert-nbsp-aria = Insert non-breaking space
ribbon-insert-hrule-aria = Insert horizontal rule
publish-print-aria = Print this document
publish-print-label = Print
ribbon-list-bullet-aria = Bullet list
ribbon-list-numbered-aria = Numbered list
ribbon-list-indent-aria = Increase list level (Tab)
ribbon-list-outdent-aria = Decrease list level (Shift+Tab)

# Styles group (Home tab)
ribbon-group-styles = Styles
ribbon-style-select-aria = Paragraph style
ribbon-style-picker-heading = Paragraph Styles
ribbon-style-search-placeholder = Search styles…
ribbon-style-apply-aria = Apply style: { $name }

# Character style group (Format tab)
ribbon-group-char-style = Character style
ribbon-char-style-select-aria = Character style
ribbon-char-style-picker-heading = Character Styles
ribbon-char-style-apply-aria = Apply character style: { $name }
ribbon-char-style-clear-aria = Remove the character style from the selection
# Shown on the select when the selection carries no character style, and on
# the chip that removes the reference.
ribbon-char-style-none = None
ribbon-char-style-picker-close-aria = Close character style picker
ribbon-char-style-empty = No character styles in this document yet — create them in the style panel.

# Paragraph group (Home tab)
ribbon-group-paragraph = Paragraph
ribbon-para-props-aria = Edit paragraph style
ribbon-para-props-heading = Paragraph Properties

# Format tab — Font colour and Highlight pickers
ribbon-group-font-color = Font colour
ribbon-font-color-picker-aria = Font colour
ribbon-highlight-picker-aria = Highlight colour
ribbon-color-clear = Automatic
ribbon-highlight-clear = No highlight
ribbon-color-recent = Recent
ribbon-color-document = In this document
ribbon-color-custom = Custom
ribbon-color-apply = Apply
ribbon-color-close-aria = Close colour picker
ribbon-color-red-aria = Red
ribbon-color-orange-aria = Orange
ribbon-color-yellow-aria = Yellow
ribbon-color-green-aria = Green
ribbon-color-blue-aria = Blue
ribbon-color-purple-aria = Purple
ribbon-group-highlight = Highlight
ribbon-highlight-yellow-aria = Yellow highlight
ribbon-highlight-green-aria = Green highlight
ribbon-highlight-cyan-aria = Cyan highlight
ribbon-highlight-magenta-aria = Magenta highlight
ribbon-highlight-blue-aria = Blue highlight
ribbon-highlight-red-aria = Red highlight
ribbon-highlight-dark-blue-aria = Dark blue highlight
ribbon-highlight-dark-cyan-aria = Dark cyan highlight
ribbon-highlight-dark-green-aria = Dark green highlight
ribbon-highlight-dark-magenta-aria = Dark magenta highlight
ribbon-highlight-dark-red-aria = Dark red highlight
ribbon-highlight-dark-yellow-aria = Dark yellow highlight
ribbon-highlight-dark-gray-aria = Dark grey highlight
ribbon-highlight-light-gray-aria = Light grey highlight
ribbon-highlight-black-aria = Black highlight
ribbon-highlight-white-aria = White highlight

# Style editor panel
ribbon-style-editor-heading = Edit Style
ribbon-style-new-aria = New custom style
ribbon-style-apply-changes = Apply Changes

# Insert tab (Spec 04 M4)
ribbon-tab-insert = Insert
ribbon-group-media = Media
ribbon-group-links = Links
ribbon-insert-link-aria = Insert link
ribbon-insert-image-aria = Insert image
ribbon-insert-image-filter = Images
ribbon-group-tables = Tables
ribbon-group-references = References
ribbon-insert-table-aria = Insert table
ribbon-insert-footnote-aria = Insert footnote

# Table contextual tab (Spec 04 M5, plan 4a.2) — shown only while the caret is
# inside a table.
ribbon-tab-table = Table
ribbon-group-table = Table
ribbon-group-table-rows = Rows & Columns
ribbon-table-delete-aria = Delete table
ribbon-table-row-insert-above-aria = Insert row above
ribbon-table-row-insert-aria = Insert row below
ribbon-table-row-delete-aria = Delete row
ribbon-table-col-insert-left-aria = Insert column left
ribbon-table-col-insert-aria = Insert column right
ribbon-table-col-delete-aria = Delete column

# Layout tab (Spec 04 M5, plan 4a.2)
ribbon-tab-layout = Layout
ribbon-group-orientation = Orientation
ribbon-orientation-portrait-aria = Portrait
ribbon-orientation-landscape-aria = Landscape
ribbon-group-margins = Margins
ribbon-margin-normal-aria = Normal margins
ribbon-margin-narrow-aria = Narrow margins
ribbon-margin-wide-aria = Wide margins
ribbon-group-page-size = Size
ribbon-page-a4-aria = A4
ribbon-page-letter-aria = US Letter
ribbon-group-columns = Columns
ribbon-columns-one-aria = One column
ribbon-columns-two-aria = Two columns
ribbon-columns-three-aria = Three columns

# References tab (Spec 04 M5, plan 4a.2) — table of contents
ribbon-tab-references = References
ribbon-group-toc = Table of Contents
ribbon-toc-insert-aria = Insert table of contents
ribbon-toc-update-aria = Update table of contents
# The heading text of an inserted table of contents (document content).
references-toc-title = Contents

# Review tab (Spec 04 M5, plan 4a.2) — track changes
ribbon-tab-review = Review
ribbon-group-review-track = Tracking
ribbon-track-changes-aria = Track changes
ribbon-group-review-changes = Changes
ribbon-accept-change-aria = Accept change
ribbon-reject-change-aria = Reject change
ribbon-accept-all-aria = Accept all changes
ribbon-reject-all-aria = Reject all changes

# Ribbon collapse toggle
ribbon-collapse-aria = Collapse ribbon
ribbon-expand-aria = Expand ribbon

# Ribbon collapse cascade (Spec 04 M3) — the overflow ("More") menu that holds
# groups the strip is too narrow to show in full.
ribbon-overflow-aria = More controls

# The Format tab's entry point into the character formatting dialog. Labelled
# rather than iconned: the pickers beside it would answer to the same glyph.
ribbon-group-character = Character
ribbon-character-dialog-label = Character…
ribbon-character-dialog-aria = Character formatting…
ribbon-group-page-style = Page style
ribbon-page-style-dialog-label = Page style…
ribbon-page-style-dialog-aria = Page style…
# Opens the style catalog editor panel (all five style families + document
# defaults) on the style at the caret.
ribbon-manage-styles-label = Manage styles…
ribbon-manage-styles-aria = Manage styles…
