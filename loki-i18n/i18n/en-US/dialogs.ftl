# Strings for the tabbed editor dialogs of design sections 2–7: span-level
# formatting, page style, document properties, insert link, insert table and
# publish EPUB 3.
#
# The paragraph style dialog (section 1) predates these and lives in style.ftl,
# alongside the style catalogue strings it shares.
#
# House rules followed throughout:
#   * Sentence case for labels; no trailing colons (the layout supplies them).
#   * An ellipsis on a control that opens something further.
#   * Where a control is disabled because the model has no field for it, the
#     message says so plainly rather than implying the setting was saved.

## ─────────────────────────────────────────────────────────────────────────
## Section 2 — Span-level formatting
## ─────────────────────────────────────────────────────────────────────────

span-dialog-title = Character formatting
span-dialog-close-aria = Close character formatting
span-dialog-tabs-more = More
span-dialog-apply = Apply
span-dialog-cancel = Cancel
span-dialog-reset = Reset
# Removes only the marks on the run; every style beneath is left alone.
span-dialog-clear-direct = Clear direct formatting

span-dialog-tab-font = Font
span-dialog-tab-effects = Effects
span-dialog-tab-position = Position
span-dialog-tab-highlight = Highlight
span-dialog-tab-language = Language

# The header line. Describes the selection the tabs act on, and how much of it
# is already formatted directly.
span-dialog-selection = { $chars } characters selected · { $direct } set directly
span-dialog-selection-styled = { $chars } characters selected · { $style } · { $direct } set directly

# Provenance, in the four levels the dialog distinguishes.
span-dialog-prov-direct = Set here, on this run
span-dialog-prov-char-style = { $value } — from the character style { $source }
span-dialog-prov-para-style = { $value } — from the paragraph style { $source }
span-dialog-prov-document = { $value } — the document default

span-dialog-font-family = Font
span-dialog-size = Size
span-dialog-unit-pt = pt
span-dialog-style = Style
span-dialog-bold = Bold
span-dialog-italic = Italic
span-dialog-underline = Underline
span-dialog-strikethrough = Strikethrough
span-dialog-single = Single
span-dialog-none = None
span-dialog-normal = Normal
span-dialog-case = Case
span-dialog-small-caps = Small capitals
span-dialog-all-caps = All capitals
span-dialog-letter-spacing = Letter spacing
# Named so the user can tell a substituted face from the one they picked.
span-dialog-device-face = Resolved on this device to { $name }

span-dialog-vertical-position = Vertical position
span-dialog-superscript = Superscript
span-dialog-subscript = Subscript
# Raise/lower and rotation have no field in the document model, so the controls
# are shown disabled rather than accepting input that would be discarded.
span-dialog-position-unsupported = Raise, lower and rotation are not stored by the document model yet, so these are disabled.
span-dialog-effects-unsupported = Outline, shadow, emboss and engrave are not stored by the document model yet, so these are disabled.

span-dialog-highlight-colour = Highlight
span-dialog-highlight-export = Highlights export as a named colour, so a shade picked here is rounded to the nearest one the format carries.

span-dialog-language = Language
span-dialog-language-placeholder = e.g. en-GB
span-dialog-language-note = The language tag travels with the run: it drives spelling, hyphenation and how a reader announces the text.

## ─────────────────────────────────────────────────────────────────────────
## Section 3 — Page style
## ─────────────────────────────────────────────────────────────────────────

page-dialog-title = Page style — { $name }
# Answers the first question this dialog raises for anyone arriving from the
# paragraph editor, where every property has an inheritance chain.
page-dialog-subtitle = Page styles do not inherit from one another; there is no parent to fall through to.
page-dialog-close-aria = Close page style
page-dialog-tabs-more = More
page-dialog-apply = Apply
page-dialog-cancel = Cancel
page-dialog-restore-preset = Restore

page-dialog-tab-page = Page
page-dialog-tab-margins = Margins
page-dialog-tab-columns = Columns
page-dialog-tab-header = Header
page-dialog-tab-footer = Footer
page-dialog-tab-borders = Borders

page-dialog-paper = Paper
# The way out of a preset: without this chip the width and height boxes stay
# read-only forever, because they only unlock once the size stops matching.
page-dialog-paper-custom = Custom
page-dialog-width = Width
page-dialog-height = Height
page-dialog-orientation = Orientation
page-dialog-portrait = Portrait
page-dialog-landscape = Landscape
page-dialog-preset-locked = Width and height follow the paper. Choose Custom to set them yourself.
page-dialog-origin-preset = { $name } — { $size }
page-dialog-origin-custom = Custom — { $size }
page-dialog-size-value = { $width } × { $height } { $unit }
page-dialog-impact = Applies to { $count } section(s) of this document.

page-dialog-unit-mm = mm
page-dialog-unit-cm = cm
page-dialog-unit-in = in
page-dialog-unit-pt = pt
page-dialog-unit-pc = pc

page-dialog-margins-heading = Margins
page-dialog-margin-top = Top
page-dialog-margin-bottom = Bottom
# The start/end labels follow the model's mirroring, not the screen edge.
page-dialog-margin-left = Left
page-dialog-margin-right = Right
page-dialog-margin-inner = Inner
page-dialog-margin-outer = Outer
page-dialog-mirror = Mirror margins on facing pages
page-dialog-margins-equal = All four margins are equal.
page-dialog-gutter-heading = Binding
page-dialog-gutter = Gutter
page-dialog-gutter-note = The gutter is added to the inner edge, so a bound copy keeps its full text width.

page-dialog-columns-layout = Columns
page-dialog-columns-single = One column
page-dialog-columns-count = { $count } columns
page-dialog-columns-gap = Gap
page-dialog-columns-separator = Line between columns
page-dialog-columns-note = Columns apply to the whole page style, so every section using it reflows.
page-dialog-columns-imported = This layout carries { $count } imported columns of unequal width. Changing the count here replaces them with equal ones.

page-dialog-header-enable = Header
page-dialog-footer-enable = Footer
page-dialog-header-height = Header height
page-dialog-footer-height = Footer height
page-dialog-same-first = Different first page
page-dialog-same-even = Different odd and even pages
page-dialog-band-variants = Variants
page-dialog-band-disabled = Turn the band on to set its height and variants.
page-dialog-band-content-note = The band's contents are edited in the document, not here.

page-dialog-numbering = Page numbering
page-dialog-number-format = Format
page-dialog-number-start = Start at
page-dialog-number-decimal = 1, 2, 3
page-dialog-number-lower-roman = i, ii, iii
page-dialog-number-upper-roman = I, II, III
page-dialog-number-lower-alpha = a, b, c
page-dialog-number-upper-alpha = A, B, C

page-dialog-border-style = Border
page-dialog-border-none = None
page-dialog-border-solid = Solid
page-dialog-border-dashed = Dashed
page-dialog-border-dotted = Dotted
page-dialog-border-double = Double
page-dialog-border-edges = Edges
page-dialog-border-all = All four
page-dialog-border-none-note = No page border is set.
page-dialog-border-export-note = Page borders survive ODF and OOXML. EPUB has no page box, so they are dropped there.

page-dialog-preview = Preview
page-dialog-preview-caption = { $width } × { $height } { $unit } · { $orientation } · { $mirrored }
page-dialog-preview-mirrored = mirrored margins
page-dialog-preview-not-mirrored = same margins on every page
page-dialog-text-area = Text area { $width } × { $height } { $unit }

## ─────────────────────────────────────────────────────────────────────────
## Section 4 — Document properties (metadata)
## ─────────────────────────────────────────────────────────────────────────

meta-dialog-title = Document properties
# Title and language are what the preflight raises as errors; the identifier is
# generated on first save, and creator and publisher are store conventions
# rather than specification requirements.
meta-dialog-subtitle = Title and language are required to publish an EPUB. An identifier is generated when you first save.
meta-dialog-close-aria = Close document properties
meta-dialog-tabs-more = More
meta-dialog-save = Save
meta-dialog-saved = Document properties saved
meta-dialog-cancel = Cancel

meta-dialog-tab-general = General
meta-dialog-tab-dublin-core = Dublin Core
meta-dialog-tab-identifiers = Identifiers
meta-dialog-tab-accessibility = Accessibility
meta-dialog-tab-statistics = Statistics

meta-dialog-required-badge = Required to publish
meta-dialog-missing-count = { $count } required field(s) still empty
meta-dialog-placeholder-empty = Not set
meta-dialog-placeholder-issued = YYYY-MM-DD
meta-dialog-placeholder-language = e.g. en-GB

meta-dialog-hint-language = A BCP 47 tag. Drives spelling, hyphenation and how a reader announces the document.
meta-dialog-hint-identifier = A URN, DOI or ISBN. Generated on first save if you leave it empty.
meta-dialog-hint-issued = The publication date, as an ISO 8601 date.
meta-dialog-hint-keywords = Separate keywords with commas.
meta-dialog-hint-contributors = Editors, translators and illustrators — anyone who is not the creator.
meta-dialog-hint-dc-type = The Dublin Core resource type, e.g. Text.
meta-dialog-identifier-warning = Changing the identifier makes this a different publication to anything that already has the old one.
meta-dialog-revision = Revision { $revision }

# Accessibility metadata is derived from the document, because the claims
# themselves have no field in the model yet.
meta-dialog-a11y-access-mode = Access mode
meta-dialog-a11y-textual = Textual
meta-dialog-a11y-visual = Visual — { $images } image(s)
meta-dialog-a11y-mode-textual = Textual
meta-dialog-a11y-mode-textual-visual = Textual and visual
meta-dialog-a11y-features = Accessibility features
meta-dialog-a11y-features-value = Structural navigation, reading order
meta-dialog-a11y-features-note = Derived from the document's headings and reading order.
meta-dialog-a11y-derived = Derived from { $images } image(s) and { $tables } table(s).
meta-dialog-a11y-unsupported = Accessibility claims are not stored by the document model yet, so these are read-only and recomputed from the document.

meta-dialog-stat-words = Words
meta-dialog-stat-characters = Characters
meta-dialog-stat-paragraphs = Paragraphs
meta-dialog-stat-images = Images
meta-dialog-stat-tables = Tables
meta-dialog-stat-notes = Footnotes
meta-dialog-stat-created = Created
meta-dialog-stat-modified = Modified
meta-dialog-stat-unknown = Unknown
meta-dialog-stat-note = Counted from the document as it stands, not from the last save.

## ─────────────────────────────────────────────────────────────────────────
## Section 5 — Insert link
## ─────────────────────────────────────────────────────────────────────────

link-dialog-title = Insert link
link-dialog-close-aria = Close insert link
link-dialog-insert = Insert
link-dialog-cancel = Cancel

link-dialog-kind-web = Web page
link-dialog-kind-email = Email
link-dialog-kind-file = File
link-dialog-kind-document = Place in this document
link-dialog-kind-document-short = This document

link-dialog-address-web = Address
link-dialog-address-email = Email address
link-dialog-address-file = File path
link-dialog-target = Target
link-dialog-target-heading = Heading { $level }
link-dialog-target-bookmark = Bookmark
link-dialog-no-targets = This document has no headings or bookmarks to link to yet.
link-dialog-no-matches = Nothing matches "{ $filter }".

link-dialog-display-text = Display text
link-dialog-display-text-placeholder = The text the link will show
link-dialog-description = Description
link-dialog-description-placeholder = Read out in place of the link text
# Both fields are shown disabled: the link mark stores a URL and nothing else,
# and inserting does not rewrite the selected text.
link-dialog-text-unsupported = Display text and description are not stored by the document model yet. The link keeps the text you selected.

# Inline, non-blocking validation (design note 22).
link-dialog-valid-web = Will open over { $scheme }.
link-dialog-valid-web-assumed = No scheme given, so https will be assumed.
link-dialog-valid-email = Will open a new message.
link-dialog-valid-file = Will open the file, if it is there when the link is followed.
link-dialog-valid-document = Will jump to this place in the document.
link-dialog-invalid-whitespace = An address cannot contain spaces.
link-dialog-invalid-no-host = This address has no host.
link-dialog-invalid-host = That host does not look like a host name.
link-dialog-invalid-scheme = { $scheme } is not a scheme a reader will follow.
link-dialog-invalid-email = An email address needs a name and a domain, separated by @.
link-dialog-invalid-email-domain = That domain does not look like a domain.

link-dialog-export-note = The link's appearance comes from the Hyperlink character style, so it changes everywhere at once.

## ─────────────────────────────────────────────────────────────────────────
## Section 6 — Insert table
## ─────────────────────────────────────────────────────────────────────────

table-dialog-title = Insert table
table-dialog-close-aria = Close insert table
table-dialog-insert = Insert
table-dialog-cancel = Cancel

table-dialog-rows = Rows
table-dialog-columns = Columns
table-dialog-size-summary = { $rows } × { $cols }
table-dialog-grid-aria = Drag to choose the table size
table-dialog-grid-cell-aria = { $rows } rows by { $cols } columns
table-dialog-increase-aria = One more { $axis }
table-dialog-decrease-aria = One fewer { $axis }

table-dialog-header-row = Header row
table-dialog-header-column = Header column
table-dialog-repeat-header = Repeat the header on every page
table-dialog-rows-break = Allow rows to break across pages
table-dialog-caption = Caption
table-dialog-caption-placeholder = What this table shows
table-dialog-caption-note = A caption and a header row are what make a table readable to someone who cannot see its shape.

table-dialog-width = Width
table-dialog-width-fill = Fill the text area
table-dialog-width-auto = Fit the content

table-dialog-preview-header-cell = Heading { $index }
table-dialog-preview-caption = Caption: { $caption }
table-dialog-preview-elided = … and the rest of { $rows } × { $cols }
table-dialog-unsupported = Per-cell header flags and keeping a row together are not stored by the document model yet, so those are disabled.
table-dialog-ribbon-note = Rows and columns are added and removed from the Table tab, once the table is in.

## ─────────────────────────────────────────────────────────────────────────
## Section 7 — Publish EPUB 3
## ─────────────────────────────────────────────────────────────────────────

publish-dialog-title = Publish EPUB 3
publish-dialog-subtitle = { $sections } section(s) in this document.
publish-dialog-close-aria = Close publish
publish-dialog-tabs-more = More
publish-dialog-publish = Publish
publish-dialog-cancel = Cancel

publish-dialog-tab-content = Content
publish-dialog-tab-metadata = Metadata
publish-dialog-tab-accessibility = Accessibility
publish-dialog-tab-fonts = Fonts
publish-dialog-tab-output = Output

# The preflight is standing: it is on screen the whole time, never a modal that
# appears after Publish is pressed (design note 27).
publish-dialog-preflight = Preflight
publish-dialog-preflight-summary = { $passed } passed · { $warnings } warning(s) · { $errors } error(s)
publish-dialog-open-metadata = Fix in document properties

publish-dialog-check-title-ok = Title is set.
publish-dialog-check-title-missing = No title. An EPUB package must carry one.
publish-dialog-check-language-ok = Language is set.
publish-dialog-check-language-missing = No language. An EPUB package must carry one.
publish-dialog-check-identifier-ok = Identifier is set.
publish-dialog-check-identifier-missing = No identifier yet. One is generated on first save.
publish-dialog-check-creator-ok = Creator is set.
publish-dialog-check-creator-missing = No creator. Most stores ask for one.
publish-dialog-check-publisher-ok = Publisher is set.
publish-dialog-check-publisher-missing = No publisher. Not required by the specification, but required by most stores.
publish-dialog-check-headings-ok = { $count } heading(s) for navigation.
publish-dialog-check-headings-missing = No headings, so the reader has nothing to navigate by.
publish-dialog-check-nesting-ok = Heading levels descend without a gap.
publish-dialog-check-nesting-skipped = A heading level is skipped, leaving a hole in the navigation.
publish-dialog-check-tables-ok = All { $count } table(s) have a caption and a header row.
publish-dialog-check-tables-bare = { $count } table(s) have no caption or no header row.
publish-dialog-check-images-ok = All { $count } image(s) have alternative text.
publish-dialog-check-images-bare = { $count } image(s) have no alternative text.

publish-dialog-toc-depth = Contents depth
publish-dialog-toc-depth-level = To heading { $depth }
publish-dialog-content-unsupported = Splitting at a chapter, a cover image and an NCX fallback are not supported by the writer yet, so those are disabled.

publish-dialog-metadata-note = These come from document properties. Edit them there and they change everywhere.
publish-dialog-not-set = Not set

publish-dialog-a11y-ok = Nothing in the document is missing what a reader needs.
publish-dialog-a11y-issues = { $count } accessibility warning(s) — see the preflight.
publish-dialog-a11y-unsupported = Accessibility claims for the package are not stored by the document model yet, so they cannot be declared here.

publish-dialog-fonts-embed = Embed fonts
publish-dialog-fonts-bundled = Bundled with Loki
publish-dialog-fonts-device = From this device
publish-dialog-fonts-none = This document uses no fonts beyond the defaults.
publish-dialog-fonts-all-bundled = Every face this document uses is bundled and can be embedded.
publish-dialog-fonts-cannot = Cannot be embedded — the licence travels with the device, not the file.
publish-dialog-fonts-substitution = { $count } face(s) will be substituted on a reader that lacks them.
publish-dialog-fonts-unsupported = Font embedding is not supported by the writer yet, so this is disabled.

publish-dialog-file-name = File name
publish-dialog-file-name-value = { $title }.epub
publish-dialog-untitled = Untitled
publish-dialog-location-note = The file is written beside the document.
publish-dialog-output-unsupported = Choosing a different location, and the EPUB 2 compatibility pass, are not supported by the writer yet.
