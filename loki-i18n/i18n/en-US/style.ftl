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
# One indent level of a list style
style-list-level-label = Level { $n }
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
