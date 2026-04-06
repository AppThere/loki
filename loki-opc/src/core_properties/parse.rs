// Copyright 2024-2026 AppThere
//
// Licensed under the MIT License.

//! Implements serde structural XML extraction directly utilizing derivations correctly isolating bounds dynamically loading properties efficiently.

use quick_xml::de::from_reader;
use serde::Deserialize;

use crate::error::{OpcError, OpcResult};
use crate::core_properties::CoreProperties;

#[derive(Debug, Deserialize)]
#[serde(rename = "coreProperties")]
struct CorePropsXml {
    #[serde(rename = "category")]
    category: Option<String>,
    #[serde(rename = "contentStatus")]
    content_status: Option<String>,
    #[serde(rename = "created")]
    created: Option<DateValue>,
    #[serde(rename = "creator")]
    creator: Option<String>,
    #[serde(rename = "description")]
    description: Option<String>,
    #[serde(rename = "identifier")]
    identifier: Option<String>,
    #[serde(rename = "keywords")]
    keywords: Option<String>,
    #[serde(rename = "language")]
    language: Option<String>,
    #[serde(rename = "lastModifiedBy")]
    last_modified_by: Option<String>,
    #[serde(rename = "lastPrinted")]
    last_printed: Option<String>,
    #[serde(rename = "modified")]
    modified: Option<DateValue>,
    #[serde(rename = "revision")]
    revision: Option<String>,
    #[serde(rename = "subject")]
    subject: Option<String>,
    #[serde(rename = "title")]
    title: Option<String>,
    #[serde(rename = "version")]
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DateValue {
    #[serde(rename = "$value")]
    value: String,
}

fn parse_date(s: &str) -> OpcResult<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| OpcError::DateTimeParse(e.to_string()))
}

/// Invokes standard XML parsing sequentially checking ISO date fields thoroughly bypassing complex native limits universally tracking structure strictly identifying structures securely extracting bounds successfully.
pub fn parse_core_properties(xml: &[u8]) -> OpcResult<CoreProperties> {
    let parsed: CorePropsXml = from_reader(xml).map_err(|e| OpcError::Xml(quick_xml::Error::Io(std::io::Error::other(e.to_string()).into())))?;
    
    let created = if let Some(dv) = parsed.created { Some(parse_date(&dv.value)?) } else { None };
    let modified = if let Some(dv) = parsed.modified { Some(parse_date(&dv.value)?) } else { None };
    let last_printed = if let Some(ls) = parsed.last_printed { Some(parse_date(&ls)?) } else { None };
    
    Ok(CoreProperties {
        category: parsed.category,
        content_status: parsed.content_status,
        created,
        creator: parsed.creator,
        description: parsed.description,
        identifier: parsed.identifier,
        keywords: parsed.keywords,
        language: parsed.language,
        last_modified_by: parsed.last_modified_by,
        last_printed,
        modified,
        revision: parsed.revision,
        subject: parsed.subject,
        title: parsed.title,
        version: parsed.version,
    })
}
