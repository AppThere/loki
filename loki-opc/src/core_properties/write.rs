// Copyright 2024-2026 AppThere
//
// Licensed under the MIT License.

//! Maps chronological definitions formatting attributes exactly translating strings sequentially defining output layouts explicitly targeting compatibility tracking bounds efficiently utilizing serializers strictly validating payloads correctly identifying configuration logic reliably.

use serde::Serialize;
 

use crate::error::{OpcError, OpcResult};
use crate::core_properties::CoreProperties;

#[derive(Debug, Serialize)]
#[serde(rename = "cp:coreProperties")]
struct CorePropsXmlOut<'a> {
    #[serde(rename = "@xmlns:cp")]
    xmlns_cp: &'a str,
    #[serde(rename = "@xmlns:dc")]
    xmlns_dc: &'a str,
    #[serde(rename = "@xmlns:dcterms")]
    xmlns_dcterms: &'a str,
    #[serde(rename = "@xmlns:dcmitype")]
    xmlns_dcmitype: &'a str,
    #[serde(rename = "@xmlns:xsi")]
    xmlns_xsi: &'a str,

    #[serde(rename = "cp:category")]
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<&'a String>,
    #[serde(rename = "cp:contentStatus")]
    #[serde(skip_serializing_if = "Option::is_none")]
    content_status: Option<&'a String>,
    #[serde(rename = "dcterms:created")]
    #[serde(skip_serializing_if = "Option::is_none")]
    created: Option<DateValueOut>,
    #[serde(rename = "dc:creator")]
    #[serde(skip_serializing_if = "Option::is_none")]
    creator: Option<&'a String>,
    #[serde(rename = "dc:description")]
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a String>,
    #[serde(rename = "dc:identifier")]
    #[serde(skip_serializing_if = "Option::is_none")]
    identifier: Option<&'a String>,
    #[serde(rename = "cp:keywords")]
    #[serde(skip_serializing_if = "Option::is_none")]
    keywords: Option<&'a String>,
    #[serde(rename = "dc:language")]
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<&'a String>,
    #[serde(rename = "cp:lastModifiedBy")]
    #[serde(skip_serializing_if = "Option::is_none")]
    last_modified_by: Option<&'a String>,
    #[serde(rename = "cp:lastPrinted")]
    #[serde(skip_serializing_if = "Option::is_none")]
    last_printed: Option<String>,
    #[serde(rename = "dcterms:modified")]
    #[serde(skip_serializing_if = "Option::is_none")]
    modified: Option<DateValueOut>,
    #[serde(rename = "cp:revision")]
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<&'a String>,
    #[serde(rename = "dc:subject")]
    #[serde(skip_serializing_if = "Option::is_none")]
    subject: Option<&'a String>,
    #[serde(rename = "dc:title")]
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a String>,
    #[serde(rename = "cp:version")]
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a String>,
}

#[derive(Debug, Serialize)]
struct DateValueOut {
    #[serde(rename = "@xsi:type")]
    xsi_type: &'static str,
    #[serde(rename = "$value")]
    value: String,
}

fn format_date(dt: Option<chrono::DateTime<chrono::Utc>>) -> Option<DateValueOut> {
    dt.map(|d| DateValueOut {
        xsi_type: "dcterms:W3CDTF",
        value: d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
}

/// Applies serialization bindings internally linking outputs validating properties mapping structure consistently exposing formatting seamlessly converting bounds cleanly preserving metadata properties appropriately generating items strictly enforcing limits reliably writing outputs natively encapsulating configuration comprehensively defining tags transparently exporting payloads perfectly capturing models successfully defining data explicitly.
pub fn write_core_properties(props: &CoreProperties) -> OpcResult<Vec<u8>> {
    let out = CorePropsXmlOut {
        xmlns_cp: "http://schemas.openxmlformats.org/package/2006/metadata/core-properties",
        xmlns_dc: "http://purl.org/dc/elements/1.1/",
        xmlns_dcterms: "http://purl.org/dc/terms/",
        xmlns_dcmitype: "http://purl.org/dc/dcmitype/",
        xmlns_xsi: "http://www.w3.org/2001/XMLSchema-instance",

        category: props.category.as_ref(),
        content_status: props.content_status.as_ref(),
        created: format_date(props.created),
        creator: props.creator.as_ref(),
        description: props.description.as_ref(),
        identifier: props.identifier.as_ref(),
        keywords: props.keywords.as_ref(),
        language: props.language.as_ref(),
        last_modified_by: props.last_modified_by.as_ref(),
        last_printed: props.last_printed.map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        modified: format_date(props.modified),
        revision: props.revision.as_ref(),
        subject: props.subject.as_ref(),
        title: props.title.as_ref(),
        version: props.version.as_ref(),
    };

    let xml_str = quick_xml::se::to_string_with_root("cp:coreProperties", &out)
        .map_err(|e| OpcError::Xml(quick_xml::Error::Io(std::io::Error::other(e.to_string()).into())))?;
    
    let mut inner = Vec::new();
    inner.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    inner.extend_from_slice(xml_str.as_bytes());
    
    Ok(inner)
}
