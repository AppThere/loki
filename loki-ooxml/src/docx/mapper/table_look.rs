// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Maps a DOCX `w:tblLook` (`DocxTblLook`) to the encoded `NodeAttr` string
//! stored on a table instance, via the doc-model [`TableLook`].

use loki_doc_model::style::TableLook;

use crate::docx::model::styles::DocxTblLook;

/// Encode a parsed `w:tblLook` as the `"tbllook"` attr string a `Table`
/// carries (see `TableLook::encode_attr`).
pub(crate) fn map_tbl_look(l: DocxTblLook) -> String {
    TableLook {
        first_row: l.first_row,
        last_row: l.last_row,
        first_column: l.first_column,
        last_column: l.last_column,
        horizontal_banding: l.h_band,
        vertical_banding: l.v_band,
    }
    .encode_attr()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_word_default_look() {
        // firstRow + firstColumn + hBand → "101010".
        let look = DocxTblLook {
            first_row: true,
            first_column: true,
            h_band: true,
            ..DocxTblLook::default()
        };
        assert_eq!(map_tbl_look(look), "101010");
    }

    #[test]
    fn maps_last_row_and_column_look() {
        let look = DocxTblLook {
            last_row: true,
            last_column: true,
            v_band: true,
            ..DocxTblLook::default()
        };
        assert_eq!(map_tbl_look(look), "010101");
    }
}
