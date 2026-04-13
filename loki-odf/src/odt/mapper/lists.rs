// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! List-style mapper: converts [`OdfListStyle`]s into
//! format-neutral [`ListStyle`]s and inserts them into a [`StyleCatalog`].

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle,
    NumberingScheme,
};
use loki_primitives::units::Points;

use crate::odt::model::list_styles::{OdfListLevelKind, OdfListStyle};
use crate::version::OdfVersion;
use crate::xml_util::parse_length;

/// Convert all list styles in `sheet` and insert them into `catalog`.
///
/// Indentation is mapped using either the ODF 1.2+ label-alignment model
/// (when [`OdfVersion::supports_label_alignment`] returns `true` and
/// `label_followed_by` is `Some`) or the legacy ODF 1.1 model.
pub(crate) fn map_list_styles(
    list_styles: &[OdfListStyle],
    catalog: &mut StyleCatalog,
    version: OdfVersion,
) {
    for odf_ls in list_styles {
        let id = ListId::new(&odf_ls.name);
        let mut levels: Vec<ListLevel> = Vec::new();

        for odf_level in &odf_ls.levels {
            let level_num = odf_level.level + 1; // 0-indexed → 1-indexed ODF

            let kind = map_level_kind(&odf_level.kind, level_num);

            let (indent_start, hanging_indent) =
                map_indentation(odf_level, version);

            // Label char props from ODF text props on the level element
            let char_props = odf_level
                .text_props
                .as_ref()
                .map(|tp| crate::odt::mapper::props::map_text_props(tp))
                .unwrap_or_default();

            levels.push(ListLevel {
                level: odf_level.level,
                kind,
                indent_start,
                hanging_indent,
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: odf_level
                    .list_tab_stop_position
                    .as_deref()
                    .and_then(parse_length),
                char_props,
            });
        }

        catalog.list_styles.insert(
            id.clone(),
            ListStyle {
                id,
                display_name: None,
                levels,
                extensions: ExtensionBag::default(),
            },
        );
    }
}

/// Map an [`OdfListLevelKind`] to the format-neutral [`ListLevelKind`].
fn map_level_kind(kind: &OdfListLevelKind, level_num: u8) -> ListLevelKind {
    match kind {
        OdfListLevelKind::Bullet { char, style_name } => {
            let bullet_char = map_bullet_char(char);
            let font = style_name.clone();
            ListLevelKind::Bullet { char: bullet_char, font }
        }
        OdfListLevelKind::Number {
            num_format,
            num_prefix,
            num_suffix,
            start_value,
            display_levels,
            ..
        } => {
            let raw_fmt = num_format.as_deref().unwrap_or("");
            if raw_fmt.is_empty() {
                return ListLevelKind::None;
            }
            let scheme = map_numbering_scheme(raw_fmt);
            let start = start_value.unwrap_or(1);
            let dl = *display_levels;
            let format =
                build_format_string(level_num, dl, num_prefix, num_suffix);
            ListLevelKind::Numbered {
                scheme,
                start_value: start,
                format,
                display_levels: dl,
            }
        }
        OdfListLevelKind::None => ListLevelKind::None,
    }
}

/// Map an ODF bullet character string to [`BulletChar`].
///
/// Uses the first Unicode scalar of the string; falls back to `'•'` for
/// an empty string.
fn map_bullet_char(s: &str) -> BulletChar {
    BulletChar::Char(s.chars().next().unwrap_or('•'))
}

/// Map an ODF `style:num-format` value to [`NumberingScheme`].
fn map_numbering_scheme(s: &str) -> NumberingScheme {
    match s {
        "1" => NumberingScheme::Decimal,
        "a" => NumberingScheme::LowerAlpha,
        "A" => NumberingScheme::UpperAlpha,
        "i" => NumberingScheme::LowerRoman,
        "I" => NumberingScheme::UpperRoman,
        _ => NumberingScheme::Decimal, // fallback
    }
}

/// Build the format string for a numbered level.
///
/// For `display_levels = 1`: `{prefix}%{level_num}{suffix}`
/// For `display_levels > 1`: `%{start}.%{start+1}...%{level_num}{suffix}`
/// where `start = level_num - display_levels + 1`.
fn build_format_string(
    level_num: u8,
    display_levels: u8,
    prefix: &Option<String>,
    suffix: &Option<String>,
) -> String {
    let suffix_str = suffix.as_deref().unwrap_or("");
    if display_levels <= 1 {
        let prefix_str = prefix.as_deref().unwrap_or("");
        format!("{prefix_str}%{level_num}{suffix_str}")
    } else {
        let start =
            (level_num as u16).saturating_sub(display_levels as u16 - 1)
                as u8;
        let mut s = String::new();
        for l in start..=level_num {
            s.push('%');
            s.push_str(&l.to_string());
            if l < level_num {
                s.push('.');
            }
        }
        s.push_str(suffix_str);
        s
    }
}

/// Map positioning attributes to `(indent_start, hanging_indent)`.
///
/// Uses the ODF 1.2+ label-alignment model when the version supports it
/// and `label_followed_by` is `Some`; otherwise falls back to the legacy
/// ODF 1.1 `text:space-before` / `text:min-label-width` model.
fn map_indentation(
    level: &crate::odt::model::list_styles::OdfListLevel,
    version: OdfVersion,
) -> (Points, Points) {
    let zero = Points::new(0.0);

    if version.supports_label_alignment()
        && level.label_followed_by.is_some()
    {
        // ODF 1.2+ label-alignment model
        let indent_start =
            level.margin_left.as_deref().and_then(parse_length).unwrap_or(zero);
        // text_indent is negative (hanging); store positive
        let hanging = level
            .text_indent
            .as_deref()
            .and_then(parse_length)
            .map(|p| {
                if p.value() < 0.0 {
                    Points::new(-p.value())
                } else {
                    p
                }
            })
            .unwrap_or(zero);
        (indent_start, hanging)
    } else {
        // ODF 1.1 legacy model: space_before + min_label_width = total indent
        let space_before = level
            .legacy_space_before
            .as_deref()
            .and_then(parse_length)
            .unwrap_or(zero);
        let label_width = level
            .legacy_min_label_width
            .as_deref()
            .and_then(parse_length)
            .unwrap_or(zero);
        let indent_start =
            Points::new(space_before.value() + label_width.value());
        let hanging = label_width;
        (indent_start, hanging)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::odt::model::list_styles::{OdfListLevel, OdfListLevelKind, OdfListStyle};
    use loki_doc_model::style::list_style::{BulletChar, ListLevelKind, NumberingScheme};

    fn bullet_level(ch: &str, legacy_space: &str, legacy_width: &str) -> OdfListLevel {
        OdfListLevel {
            level: 0,
            kind: OdfListLevelKind::Bullet {
                char: ch.into(),
                style_name: None,
            },
            legacy_space_before: Some(legacy_space.into()),
            legacy_min_label_width: Some(legacy_width.into()),
            legacy_min_label_distance: None,
            label_followed_by: None,
            list_tab_stop_position: None,
            text_indent: None,
            margin_left: None,
            text_props: None,
        }
    }

    fn number_level(
        fmt: &str,
        suffix: &str,
        start: u32,
        display_levels: u8,
        margin: &str,
        indent: &str,
    ) -> OdfListLevel {
        OdfListLevel {
            level: 0,
            kind: OdfListLevelKind::Number {
                num_format: Some(fmt.into()),
                num_prefix: None,
                num_suffix: Some(suffix.into()),
                start_value: Some(start),
                display_levels,
                style_name: None,
            },
            legacy_space_before: None,
            legacy_min_label_width: None,
            legacy_min_label_distance: None,
            label_followed_by: Some("listtab".into()),
            list_tab_stop_position: None,
            text_indent: Some(indent.into()),
            margin_left: Some(margin.into()),
            text_props: None,
        }
    }

    #[test]
    fn bullet_char_bullet() {
        let level = bullet_level("•", "0.25cm", "0.25cm");
        let ls = OdfListStyle { name: "L1".into(), levels: vec![level] };
        let mut catalog = StyleCatalog::new();
        map_list_styles(&[ls], &mut catalog, OdfVersion::V1_1);
        let style = catalog.list_styles.get(&ListId::new("L1")).unwrap();
        assert_eq!(style.levels.len(), 1);
        match &style.levels[0].kind {
            ListLevelKind::Bullet { char: BulletChar::Char(c), .. } => {
                assert_eq!(*c, '•');
            }
            other => panic!("expected Bullet, got {:?}", other),
        }
    }

    #[test]
    fn bullet_custom_char() {
        let level = bullet_level("-", "0.5cm", "0.25cm");
        let ls = OdfListStyle { name: "L2".into(), levels: vec![level] };
        let mut catalog = StyleCatalog::new();
        map_list_styles(&[ls], &mut catalog, OdfVersion::V1_1);
        let style = catalog.list_styles.get(&ListId::new("L2")).unwrap();
        match &style.levels[0].kind {
            ListLevelKind::Bullet { char: BulletChar::Char(c), .. } => {
                assert_eq!(*c, '-');
            }
            other => panic!("expected Bullet, got {:?}", other),
        }
    }

    #[test]
    fn number_decimal_with_suffix() {
        let level =
            number_level("1", ".", 1, 1, "1.27cm", "-0.635cm");
        let ls = OdfListStyle { name: "L3".into(), levels: vec![level] };
        let mut catalog = StyleCatalog::new();
        map_list_styles(&[ls], &mut catalog, OdfVersion::V1_2);
        let style = catalog.list_styles.get(&ListId::new("L3")).unwrap();
        match &style.levels[0].kind {
            ListLevelKind::Numbered { scheme, format, start_value, .. } => {
                assert_eq!(*scheme, NumberingScheme::Decimal);
                assert_eq!(format, "%1.");
                assert_eq!(*start_value, 1);
            }
            other => panic!("expected Numbered, got {:?}", other),
        }
    }

    #[test]
    fn number_lower_alpha() {
        let level =
            number_level("a", ")", 1, 1, "1.27cm", "-0.635cm");
        let ls = OdfListStyle { name: "L4".into(), levels: vec![level] };
        let mut catalog = StyleCatalog::new();
        map_list_styles(&[ls], &mut catalog, OdfVersion::V1_2);
        let style = catalog.list_styles.get(&ListId::new("L4")).unwrap();
        match &style.levels[0].kind {
            ListLevelKind::Numbered { scheme, format, .. } => {
                assert_eq!(*scheme, NumberingScheme::LowerAlpha);
                assert_eq!(format, "%1)");
            }
            other => panic!("expected Numbered, got {:?}", other),
        }
    }

    #[test]
    fn odf12_label_alignment_indentation() {
        let level = number_level("1", ".", 1, 1, "1.27cm", "-0.635cm");
        let ls = OdfListStyle { name: "L5".into(), levels: vec![level] };
        let mut catalog = StyleCatalog::new();
        map_list_styles(&[ls], &mut catalog, OdfVersion::V1_2);
        let style = catalog.list_styles.get(&ListId::new("L5")).unwrap();
        let ll = &style.levels[0];
        // margin_left = 1.27cm ≈ 36.0pt
        assert!(
            ll.indent_start.value() > 35.0 && ll.indent_start.value() < 37.0,
            "indent_start={}", ll.indent_start.value()
        );
        // text_indent = -0.635cm ≈ 18pt, stored as positive hanging
        assert!(
            ll.hanging_indent.value() > 17.0 && ll.hanging_indent.value() < 19.0,
            "hanging_indent={}", ll.hanging_indent.value()
        );
    }

    #[test]
    fn odf11_legacy_indentation() {
        // space_before=0.25cm, min_label_width=0.25cm
        // → indent_start = 0.5cm, hanging = 0.25cm
        let level = bullet_level("•", "0.25cm", "0.25cm");
        let ls = OdfListStyle { name: "L6".into(), levels: vec![level] };
        let mut catalog = StyleCatalog::new();
        map_list_styles(&[ls], &mut catalog, OdfVersion::V1_1);
        let style = catalog.list_styles.get(&ListId::new("L6")).unwrap();
        let ll = &style.levels[0];
        let expected_indent = parse_length("0.5cm").unwrap().value();
        let expected_hanging = parse_length("0.25cm").unwrap().value();
        assert!(
            (ll.indent_start.value() - expected_indent).abs() < 1e-4,
            "indent_start: expected {:.3}, got {:.3}", expected_indent, ll.indent_start.value()
        );
        assert!(
            (ll.hanging_indent.value() - expected_hanging).abs() < 1e-4,
            "hanging: expected {:.3}, got {:.3}", expected_hanging, ll.hanging_indent.value()
        );
    }

    #[test]
    fn display_levels_two_format() {
        // level=1 (0-indexed), display_levels=2
        // → format "%1.%2."
        let level = OdfListLevel {
            level: 1, // 0-indexed → level_num=2
            kind: OdfListLevelKind::Number {
                num_format: Some("1".into()),
                num_prefix: None,
                num_suffix: Some(".".into()),
                start_value: Some(1),
                display_levels: 2,
                style_name: None,
            },
            legacy_space_before: None,
            legacy_min_label_width: None,
            legacy_min_label_distance: None,
            label_followed_by: Some("listtab".into()),
            list_tab_stop_position: None,
            text_indent: Some("-0.5cm".into()),
            margin_left: Some("1cm".into()),
            text_props: None,
        };
        let ls = OdfListStyle { name: "L7".into(), levels: vec![level] };
        let mut catalog = StyleCatalog::new();
        map_list_styles(&[ls], &mut catalog, OdfVersion::V1_2);
        let style = catalog.list_styles.get(&ListId::new("L7")).unwrap();
        match &style.levels[0].kind {
            ListLevelKind::Numbered { format, display_levels, .. } => {
                assert_eq!(format, "%1.%2.");
                assert_eq!(*display_levels, 2);
            }
            other => panic!("expected Numbered, got {:?}", other),
        }
    }
}
