// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF field → [`Field`] mapping.

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};

use crate::odt::model::fields::OdfField;

pub(crate) fn map_field(odf: &OdfField) -> Field {
    let kind = match odf {
        OdfField::PageNumber { .. } => FieldKind::PageNumber,
        OdfField::PageCount => FieldKind::PageCount,
        OdfField::Date { data_style, .. } => FieldKind::Date {
            format: data_style.clone(),
        },
        OdfField::Time { data_style, .. } => FieldKind::Time {
            format: data_style.clone(),
        },
        OdfField::Title => FieldKind::Title,
        OdfField::Subject => FieldKind::Subject,
        OdfField::AuthorName => FieldKind::Author,
        OdfField::FileName { .. } => FieldKind::FileName,
        OdfField::WordCount => FieldKind::WordCount,
        OdfField::CrossReference { ref_name, display } => {
            let format = match display.as_deref() {
                Some("number") => CrossRefFormat::Number,
                Some("page") => CrossRefFormat::Page,
                Some("caption") => CrossRefFormat::Caption,
                _ => CrossRefFormat::HeadingText,
            };
            FieldKind::CrossReference {
                target: ref_name.clone(),
                format,
            }
        }
        OdfField::ChapterName { display_levels } => FieldKind::Raw {
            instruction: format!("chapter display-levels={display_levels}"),
        },
        OdfField::Unknown { local_name, .. } => FieldKind::Raw {
            instruction: local_name.clone(),
        },
    };
    Field {
        kind,
        current_value: None,
        extensions: ExtensionBag::default(),
    }
}
