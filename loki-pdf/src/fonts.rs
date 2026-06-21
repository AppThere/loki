// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Font collection and `CIDFontType2` embedding for the PDF exporter.
//!
//! Every distinct face used by the layout (keyed by its font-data pointer and
//! collection index) is embedded as a Type0 → CIDFontType2 program with
//! `Identity-H` encoding, so the content stream can address glyphs by raw
//! glyph id. PDF/X requires every font to be embedded; this module fulfils
//! that by writing the full font program as a `FontFile2` stream.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Finish, Name, Pdf, Ref, Str};

use crate::error::PdfError;

/// Identity registry/ordering used for the descendant CID font.
const CID_SYSTEM_INFO: SystemInfo = SystemInfo {
    registry: Str(b"Adobe"),
    ordering: Str(b"Identity"),
    supplement: 0,
};

/// A single embedded face plus the set of glyph ids referenced by the content.
pub struct Face {
    /// Resource name used in page resource dictionaries (e.g. `F0`).
    pub resource: String,
    data: Arc<Vec<u8>>,
    font_index: u32,
    used_glyphs: BTreeSet<u16>,
}

/// The indirect references allocated for one embedded face.
#[derive(Clone, Copy)]
pub struct FaceRefs {
    /// The composite Type0 font dictionary (named in page resources).
    pub type0: Ref,
    cid: Ref,
    descriptor: Ref,
    font_file: Ref,
    to_unicode: Ref,
}

/// Collects the distinct faces used while content streams are built.
#[derive(Default)]
pub struct FontBank {
    faces: Vec<Face>,
    index_by_key: HashMap<(usize, u32), usize>,
}

impl FontBank {
    /// Creates an empty bank.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `data`/`font_index` (if new), records the glyph ids as used,
    /// and returns the resource name to reference in the content stream.
    pub fn use_face<I>(&mut self, data: &Arc<Vec<u8>>, font_index: u32, glyphs: I) -> String
    where
        I: IntoIterator<Item = u16>,
    {
        let key = (Arc::as_ptr(data) as *const u8 as usize, font_index);
        let idx = *self.index_by_key.entry(key).or_insert_with(|| {
            let resource = format!("F{}", self.faces.len());
            self.faces.push(Face {
                resource,
                data: Arc::clone(data),
                font_index,
                used_glyphs: BTreeSet::new(),
            });
            self.faces.len() - 1
        });
        let face = &mut self.faces[idx];
        face.used_glyphs.extend(glyphs);
        face.resource.clone()
    }

    /// All registered faces, in resource-name order.
    #[must_use]
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// `true` when no faces were registered (document had no text).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Allocates the five indirect references each face needs, advancing
    /// `next`. Returns them aligned with [`Self::faces`].
    pub fn allocate_refs(&self, next: &mut i32) -> Vec<FaceRefs> {
        self.faces
            .iter()
            .map(|_| {
                let mut take = || {
                    let r = Ref::new(*next);
                    *next += 1;
                    r
                };
                FaceRefs {
                    type0: take(),
                    cid: take(),
                    descriptor: take(),
                    font_file: take(),
                    to_unicode: take(),
                }
            })
            .collect()
    }

    /// Writes every font object (Type0, CIDFontType2, descriptor, font file,
    /// and ToUnicode CMap) into `pdf`.
    pub fn embed(&self, pdf: &mut Pdf, refs: &[FaceRefs]) -> Result<(), PdfError> {
        for (face, r) in self.faces.iter().zip(refs) {
            embed_face(pdf, face, r)?;
        }
        Ok(())
    }
}

fn embed_face(pdf: &mut Pdf, face: &Face, r: &FaceRefs) -> Result<(), PdfError> {
    let ttf = ttf_parser::Face::parse(&face.data, face.font_index)
        .map_err(|e| PdfError::Font(format!("parse: {e:?}")))?;
    let upem = f32::from(ttf.units_per_em().max(1));
    let scale = 1000.0 / upem;
    let base_font = Name(b"LokiEmbedded");

    // Composite (Type0) font.
    pdf.type0_font(r.type0)
        .base_font(base_font)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(r.cid)
        .to_unicode(r.to_unicode);

    // Descendant CIDFontType2 with Identity CID→GID mapping.
    {
        let mut cid = pdf.cid_font(r.cid);
        cid.subtype(CidFontType::Type2)
            .base_font(base_font)
            .system_info(CID_SYSTEM_INFO)
            .font_descriptor(r.descriptor)
            .cid_to_gid_map_predefined(Name(b"Identity"))
            .default_width(0.0);
        let mut widths = cid.widths();
        for &gid in &face.used_glyphs {
            let adv = ttf.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap_or(0);
            widths.consecutive(gid, [f32::from(adv) * scale]);
        }
        widths.finish();
        cid.finish();
    }

    // Font descriptor with metrics + the embedded program.
    let bbox = ttf.global_bounding_box();
    let mut flags = FontFlags::NON_SYMBOLIC;
    if ttf.is_italic() {
        flags |= FontFlags::ITALIC;
    }
    if ttf.is_monospaced() {
        flags |= FontFlags::FIXED_PITCH;
    }
    pdf.font_descriptor(r.descriptor)
        .name(base_font)
        .flags(flags)
        .bbox(pdf_writer::Rect::new(
            f32::from(bbox.x_min) * scale,
            f32::from(bbox.y_min) * scale,
            f32::from(bbox.x_max) * scale,
            f32::from(bbox.y_max) * scale,
        ))
        .italic_angle(ttf.italic_angle())
        .ascent(f32::from(ttf.ascender()) * scale)
        .descent(f32::from(ttf.descender()) * scale)
        .cap_height(ttf.capital_height().map_or(0.0, f32::from) * scale)
        .stem_v(80.0)
        .font_file2(r.font_file);

    // The full font program, embedded uncompressed (Length1 = byte length).
    pdf.stream(r.font_file, &face.data)
        .pair(Name(b"Length1"), face.data.len() as i32);

    // ToUnicode mapping built from the face's cmap so extracted text is
    // searchable / copyable. Stored uncompressed (no Filter).
    let cmap = build_to_unicode(&ttf, &face.used_glyphs);
    pdf.stream(r.to_unicode, &cmap);
    Ok(())
}

/// Builds a ToUnicode CMap mapping the used glyph ids back to characters using
/// the font's own cmap subtables (reverse lookup).
fn build_to_unicode(ttf: &ttf_parser::Face, used: &BTreeSet<u16>) -> Vec<u8> {
    let mut gid_to_char: HashMap<u16, char> = HashMap::new();
    if let Some(subtables) = ttf.tables().cmap {
        for subtable in subtables.subtables {
            if !subtable.is_unicode() {
                continue;
            }
            subtable.codepoints(|cp| {
                if let Some(ch) = char::from_u32(cp)
                    && let Some(gid) = subtable.glyph_index(cp)
                {
                    gid_to_char.entry(gid.0).or_insert(ch);
                }
            });
        }
    }
    let mut cmap = UnicodeCmap::new(Name(b"Adobe-Identity-UCS"), CID_SYSTEM_INFO);
    for &gid in used {
        if let Some(&ch) = gid_to_char.get(&gid) {
            cmap.pair(gid, ch);
        }
    }
    cmap.finish().to_vec()
}
