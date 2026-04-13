// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: MIT

//! Open Packaging Conventions (ISO/IEC 29500-2:2021) primary root struct definitions.

use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::Path;

use crate::{
    content_types::ContentTypeMap,
    core_properties::CoreProperties,
    error::{DeviationWarning, OpcResult},
    part::{PartData, PartName},
    relationships::RelationshipSet,
};

/// An OPC package. The primary entry point for this crate.
///
/// A package corresponds to a ZIP file containing parts, relationships,
/// content types, and optional core properties, as specified in
/// ISO/IEC 29500-2:2021.
pub struct Package {
    parts: HashMap<PartName, PartData>,
    relationships: RelationshipSet,
    part_relationships: HashMap<PartName, RelationshipSet>,
    content_type_map: ContentTypeMap,
    core_properties: Option<CoreProperties>,
    thumbnail: Option<PartData>,
    has_digital_signatures: bool,
    pub(crate) warnings: Vec<DeviationWarning>,
}

impl Default for Package {
    fn default() -> Self {
        Self::new()
    }
}

impl Package {
    /// Create a new empty package.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parts: HashMap::new(),
            relationships: RelationshipSet::default(),
            part_relationships: HashMap::new(),
            content_type_map: ContentTypeMap::default(),
            core_properties: None,
            thumbnail: None,
            has_digital_signatures: false,
            warnings: Vec::new(),
        }
    }

    /// Open a package from a reader.
    ///
    /// Reads the ZIP structure, validates [Content_Types].xml and all
    /// relationship parts, and resolves all part names per §7.3.
    /// Deviation handling is applied automatically unless the `strict`
    /// feature is enabled.
    pub fn open(mut reader: impl Read + Seek) -> OpcResult<Self> {
        // ZIP handling is deferred to the reader module internally,
        // which parses `[Content_Types].xml` and subsequent `.rels` metadata files natively.
        crate::zip::read::read_package_from_zip(&mut reader)
    }

    /// Open a package from a file path (requires `std` feature).
    #[cfg(feature = "std")]
    pub fn open_path(path: impl AsRef<Path>) -> OpcResult<Self> {
        let file = std::fs::File::open(path)?;
        Self::open(std::io::BufReader::new(file))
    }

    /// Write the package to a writer, producing a spec-conformant ZIP.
    /// Written output always conforms to ISO/IEC 29500-2:2021 regardless
    /// of the deviation settings used on read.
    pub fn write(&self, mut writer: impl Write + Seek) -> OpcResult<()> {
        crate::zip::write::write_package_to_zip(self, &mut writer)
    }

    /// Write the package to a file path (requires `std` feature).
    #[cfg(feature = "std")]
    pub fn write_path(&self, path: impl AsRef<Path>) -> OpcResult<()> {
        let file = std::fs::File::create(path)?;
        self.write(std::io::BufWriter::new(file))
    }

    // --- Parts ---

    /// Return the part data for the given part name, or None if absent.
    pub fn part(&self, name: &PartName) -> Option<&PartData> {
        self.parts.get(name)
    }

    /// Return a mutable reference to the part data for the given name.
    pub fn part_mut(&mut self, name: &PartName) -> Option<&mut PartData> {
        self.parts.get_mut(name)
    }

    /// Insert or replace a part.
    pub fn set_part(&mut self, name: PartName, data: PartData) {
        self.parts.insert(name, data);
    }

    /// Remove a part and its relationships part if present.
    pub fn remove_part(&mut self, name: &PartName) -> Option<PartData> {
        self.part_relationships.remove(name);
        self.parts.remove(name)
    }

    /// Iterate over all part names in the package.
    pub fn part_names(&self) -> impl Iterator<Item = &PartName> {
        self.parts.keys()
    }
    
    /// Package-level parts map strictly for read operations natively.
    pub(crate) fn parts_map(&self) -> &HashMap<PartName, PartData> {
        &self.parts
    }

    // --- Relationships ---

    /// Package-level relationships (/_rels/.rels).
    pub fn relationships(&self) -> &RelationshipSet {
        &self.relationships
    }

    /// Mutable package-level relationships.
    pub fn relationships_mut(&mut self) -> &mut RelationshipSet {
        &mut self.relationships
    }

    /// Relationships for a specific part, if a relationships part exists.
    pub fn part_relationships(&self, name: &PartName) -> Option<&RelationshipSet> {
        self.part_relationships.get(name)
    }

    /// Mutable relationships for a specific part.
    pub fn part_relationships_mut(&mut self, name: &PartName) -> &mut RelationshipSet {
        self.part_relationships.entry(name.clone()).or_default()
    }

    // --- Content Types ---

    /// Resolve the media type for a part name per §7.3.7.
    /// Applies Override first, then Default, then returns None.
    pub fn content_type(&self, name: &PartName) -> Option<&str> {
        self.content_type_map.resolve(name)
    }

    /// The underlying content type map.
    pub fn content_type_map(&self) -> &ContentTypeMap {
        &self.content_type_map
    }

    /// Mutable content type map.
    pub fn content_type_map_mut(&mut self) -> &mut ContentTypeMap {
        &mut self.content_type_map
    }

    // --- Core Properties ---

    /// The package's core properties, if present (§8).
    pub fn core_properties(&self) -> Option<&CoreProperties> {
        self.core_properties.as_ref()
    }

    /// Mutable core properties. Creates the part and relationship if absent.
    pub fn core_properties_mut(&mut self) -> &mut CoreProperties {
        if self.core_properties.is_none() {
            self.core_properties = Some(CoreProperties::default());
        }
        self.core_properties.as_mut().unwrap()
    }

    // --- Thumbnails ---

    /// The thumbnail part, if present (§9).
    pub fn thumbnail(&self) -> Option<&PartData> {
        self.thumbnail.as_ref()
    }

    /// Set the thumbnail part.
    pub fn set_thumbnail(&mut self, data: PartData, media_type: &str) {
        self.content_type_map_mut()
            .add_default(media_type, media_type);
        self.thumbnail = Some(data);
    }

    // --- Digital Signatures ---

    /// Returns true if the package contains digital signature parts (§10).
    /// Signature content cannot be read or written in v0.1.0.
    pub fn has_digital_signatures(&self) -> bool {
        self.has_digital_signatures
    }

    pub(crate) fn set_has_digital_signatures(&mut self, has: bool) {
        self.has_digital_signatures = has;
    }

    // --- Diagnostics ---

    /// Deviation warnings accumulated during open(). Empty in strict mode.
    pub fn deviation_warnings(&self) -> &[DeviationWarning] {
        &self.warnings
    }
}
