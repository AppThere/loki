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
