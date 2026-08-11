# Style management panel (Spec 05)

# Resolved-vs-overridden inspector (M2)
style-inspector-heading = Where properties come from
style-inspector-unset = —

# Property labels
style-prop-font-family = Font
style-prop-font-size = Size
style-prop-bold = Bold
style-prop-italic = Italic
style-prop-alignment = Alignment
style-prop-indent-start = Indent (start)
style-prop-indent-end = Indent (end)
style-prop-indent-first-line = First-line indent
style-prop-space-before = Space before
style-prop-space-after = Space after
style-prop-line-height = Line height

# Reset-to-inherited control (shown on locally-set rows)
style-reset-aria = Reset to inherited
# Jump-to-ancestor link (shown on inherited rows)
style-jump-aria = Open the source style
# Pending (uncommitted draft edit) marker
style-staged-title = Pending — apply to commit
# Impact preview: dependent styles a staged change will also change
style-impact-preview = Applying also changes { $count } dependent style(s): { $names }
# Linked character style section heading (§9 linked family)
style-linked-heading = Linked character style · { $name }
# Character-styles family list heading (§9 character family)
style-char-family-heading = Character styles
# Editable character-style form heading (M6 character family, 4a.3)
style-char-form-heading = Edit character style
# Compact breadcrumb drill-down heading — the selected style's substyles (M7)
style-tree-substyles-heading = Substyles
# List-styles family list heading (§9 list family, non-inheriting)
style-list-family-heading = List styles
# Page-styles family (§9 page family, non-inheriting; ADR-0012 Decision 2)
style-page-family-heading = Page styles
# Rename field for the selected page style (LibreOffice-style named page styles)
style-page-name-label = Name
style-page-rename = Rename
style-page-size = Size
# Custom (user-defined) page size entry — width x height. The unit suffix is
# not a string: it comes from MeasurementUnit::abbreviation (T6.4).
style-page-size-custom = Custom
style-page-size-apply = Set
style-page-orientation = Orientation
style-page-margins = Margins
style-page-columns = Columns
style-page-columns-fewer = −
style-page-columns-more = +
style-page-column-separator = Separator
style-page-new = New
style-page-unapplied = { $name } (unused)
style-page-apply-label = Apply
style-page-apply-here = Apply to this section
# App-scoped defaults for new documents (T6.3) and the measurement unit (T6.4)
style-page-defaults-label = New docs
style-page-set-default = Use as default
style-page-clear-default = Reset
style-page-unit-label = Unit
style-page-duplicate = Duplicate
style-page-delete = Delete
style-page-margins-custom = Exact
style-page-margin-top = Top
style-page-margin-bottom = Bottom
style-page-margin-left = Left
style-page-margin-right = Right
style-page-margins-apply = Set
# One indent level of a list style
style-list-level-label = Level { $n }
# §10 tier 5 — the per-level list edit form
style-list-form-heading = Edit level { $n }
style-list-kind-bullet = Bullet
style-list-kind-numbered = Numbered
style-list-format-label = Format
style-list-start-label = Start
style-list-bullet-label = Character
style-list-indent-label = Indent (pt)
style-list-hanging-label = Hanging (pt)
# A list level's label kind and geometry (non-inheriting; shown read-only)
style-list-level-detail = { $label } · indent { $indent } · hanging { $hanging } · { $alignment }
# Compact (< 600 px) segmented switcher between the edit and inspect panes (§11)
style-section-edit = Edit
style-section-inspect = Inspect
# Re-parenting rejected because it would create an inheritance cycle
style-reparent-cycle = Cannot set that parent: it would create an inheritance cycle
# Delete control (user styles only; built-in styles are protected)
style-delete-label = Delete
style-delete-aria = Delete this style
style-delete-success = Style deleted; { $count } child style(s) re-parented
style-delete-builtin = Built-in styles can't be deleted

# Provenance labels
style-provenance-local = Local
style-provenance-inherited = Inherited · { $ancestor }
style-provenance-default = Default
style-provenance-engine = Auto

# ── Table family (Spec 05 M6, 4a.3) ─────────────────────────────────────────
style-table-family-heading = Table styles
style-table-form-heading = Edit table style
style-table-row-band-label = Row bands
style-table-col-band-label = Column bands
style-table-align-unset = Auto
style-table-align-left = Left
style-table-align-center = Center
style-table-align-right = Right

# ─────────────────────────────────────────────────────────────────────────────
# Paragraph style editor dialog (Spec 05 M2/M6, design section 1)
# ─────────────────────────────────────────────────────────────────────────────

style-dialog-title = Paragraph style — { $name }
style-dialog-close-aria = Close the paragraph style editor
style-dialog-tabs-more = More
style-dialog-apply = Apply
style-dialog-cancel = Cancel
style-dialog-reset-all = Reset all to inherited
style-dialog-preview = Preview

# Tab labels
style-dialog-tab-general = General
style-dialog-tab-font = Font
style-dialog-tab-indents = Indents & spacing
style-dialog-tab-alignment = Alignment
style-dialog-tab-text-flow = Text flow
style-dialog-tab-borders = Borders
style-dialog-tab-tab-stops = Tab stops

# Provenance lines — the line under every control saying where its value
# comes from. `-bare` variants are used when the value has no display form.
style-dialog-prov-local = Set on this style
style-dialog-prov-inherited = Inherited from { $source } · { $value }
style-dialog-prov-inherited-bare = Inherited from { $source }
style-dialog-prov-document = Document default · { $value }
style-dialog-prov-document-bare = Document default
style-dialog-prov-engine = Engine default · { $value }
style-dialog-prov-engine-bare = Engine default
style-dialog-prov-source-unnamed = an unnamed style
style-dialog-prov-edit-there = Edit there
style-dialog-prov-reset = Reset

# Units
style-dialog-unit-pt = { $value } pt
style-dialog-unit-pt-short = pt
style-dialog-unit-pt-at-least = at least { $value } pt
style-dialog-unit-multiple = { $value }×
style-dialog-unit-multiple-short = ×
style-dialog-unit-lines = lines
style-dialog-unit-lines-value = { $value } lines

# General tab
style-dialog-general-name = Name
style-dialog-general-inherits-from = Inherits from
style-dialog-general-next = Next paragraph uses
style-dialog-general-chain = Inheritance chain
style-dialog-general-counts = { $local } local · { $inherited } inherited
style-dialog-general-reresolve = Changing this re-resolves { $count } inherited properties
style-dialog-general-dependents =
    { $count ->
        [0] No other style is based on this one.
       *[other] { $count } style(s) are based on this one: { $names }
    }
style-dialog-general-builtin = This is a built-in style. Its name is part of the mapping used when importing and exporting, so renaming it can change how the document round-trips.

# Font tab
style-dialog-font-family = Font family
style-dialog-font-family-inherit = Inherited
style-dialog-font-bundled-heading = Bundled with Loki · always available
style-dialog-font-bundled-badge = Bundled
style-dialog-font-device-heading = Installed on this device · { $count }
style-dialog-font-no-matches = No font matches that name.
style-dialog-font-size = Size
style-dialog-font-posture = Weight & posture
style-dialog-font-regular = Regular
style-dialog-font-bold = Bold
style-dialog-font-italic = Italic
style-dialog-font-colour = Text colour
style-dialog-font-colour-automatic = Automatic
style-dialog-font-language = Language
style-dialog-font-language-inherit = Follows the document
style-dialog-font-local-count = { $count } propert{ $count ->
        [one] y is
       *[other] ies are
    } set on this style. Changing an ancestor will not affect { $count ->
        [one] it
       *[other] them
    }.

# Indents & spacing tab
style-dialog-indents-heading = Indents
style-dialog-indents-before = Before text
style-dialog-indents-after = After text
style-dialog-indents-first-line = First line
style-dialog-spacing-heading = Spacing
style-dialog-spacing-above = Above
style-dialog-spacing-below = Below
style-dialog-spacing-line-height = Line height
style-dialog-spacing-collapse = Collapse spacing between paragraphs of the same style
style-dialog-spacing-collapse-unsupported = Contextual spacing is not stored in the document model yet, so this control is disabled rather than silently doing nothing.

# Alignment tab
style-dialog-align-horizontal = Horizontal
style-dialog-align-left = Left
style-dialog-align-centre = Centre
style-dialog-align-right = Right
style-dialog-align-justified = Justified
style-dialog-align-distributed = Distributed
style-dialog-align-distributed-note = Distributed stretches the last line of the paragraph as well; Justified leaves it short.
style-dialog-align-unsupported = Last line of a justified paragraph, text-to-text vertical alignment, snapping to the page text grid, and expanding a single word are not stored in the document model yet, so they are not offered here.

# Text flow tab
style-dialog-flow-breaks-heading = Breaks
style-dialog-flow-break-before = Start this paragraph on a new page
style-dialog-flow-break-after = Start the next paragraph on a new page
style-dialog-flow-keep-heading = Keeping together
style-dialog-flow-keep-together = Do not split this paragraph across pages
style-dialog-flow-keep-with-next = Keep with the next paragraph
style-dialog-flow-keep-together-note = While this paragraph cannot split, the orphan and widow counts below have no effect — they are kept so they apply again if you allow splitting.
style-dialog-flow-orphans = Orphan control
style-dialog-flow-widows = Widow control
style-dialog-flow-hyphenation-unsupported = Hyphenation is not stored in the document model yet — neither the automatic flag nor the character counts — so it is not offered here.

# Borders tab
style-dialog-borders-edges = Edges
style-dialog-borders-none = None
style-dialog-borders-start-only = Start edge only
style-dialog-borders-all = All four
style-dialog-borders-mixed = Mixed
style-dialog-borders-mixed-note = This style carries a border on a combination of edges the presets cannot name — imported from the document. Choosing a preset replaces it.
style-dialog-borders-line-style = Line style
style-dialog-borders-solid = Solid
style-dialog-borders-dashed = Dashed
style-dialog-borders-dotted = Dotted
style-dialog-borders-double = Double
style-dialog-borders-wave = Wave
style-dialog-borders-width = Width
style-dialog-borders-colour = Colour
style-dialog-borders-colour-automatic = Automatic
style-dialog-borders-no-edge = No edge is set, so there is no line to style.
style-dialog-borders-padding-heading = Padding to text
style-dialog-borders-padding-top = Top
style-dialog-borders-padding-bottom = Bottom
style-dialog-borders-padding-left = Left
style-dialog-borders-padding-right = Right

# Tab stops tab
style-dialog-stops-position = Position
style-dialog-stops-alignment = Alignment
style-dialog-stops-leader = Leader
style-dialog-stops-empty = No tab stops. The engine's default stops apply.
style-dialog-stops-count = { $count } stop(s)
style-dialog-stops-new-at = New stop at
style-dialog-stops-add = Add stop
style-dialog-stops-clear = Clear all
style-dialog-stops-remove-aria = Remove this tab stop
style-dialog-stops-align-left = Left
style-dialog-stops-align-right = Right
style-dialog-stops-align-centre = Centre
style-dialog-stops-align-decimal = Decimal
style-dialog-stops-align-clear = Clear
style-dialog-stops-leader-none = None
style-dialog-stops-leader-dotted = Dotted
style-dialog-stops-leader-dashed = Dashed
style-dialog-stops-leader-underscore = Underscore
style-dialog-stops-materialise-note = These stops are inherited. Editing them copies the whole list onto this style, which then stops tracking changes to its parent's stops.

# Preview specimen
style-dialog-preview-body-1 = The afternoon had the particular stillness of a room where someone has just stopped speaking, and the light came in low across the floorboards.
style-dialog-preview-body-2 = She set the cup down without a sound and waited for the next thing to happen.

# §3b — the Page dialog's Sections block
page-dialog-sections = Sections
page-dialog-section-row = Section { $n } — { $style }
page-dialog-section-unstyled = no named style
page-dialog-section-apply = Apply this style
page-dialog-section-current = Current
# §3c — columns stepper + per-column widths
page-dialog-columns-fewer = −
page-dialog-columns-more = +
page-dialog-columns-widths = Column widths
page-dialog-columns-width-n = Column { $n }
page-dialog-columns-custom-widths = Set custom widths…
page-dialog-columns-equal = Equal columns
